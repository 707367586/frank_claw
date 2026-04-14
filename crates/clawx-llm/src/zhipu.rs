//! ZhipuAI (智谱) LLM provider implementation.
//!
//! ZhipuAI uses an OpenAI-compatible Chat Completions API.
//! Base URL: https://open.bigmodel.cn/api/paas/v4
//! Models: glm-4, glm-4-flash, glm-4-plus, glm-4v, etc.

use std::pin::Pin;

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

const DEFAULT_BASE_URL: &str = "https://open.bigmodel.cn/api/paas/v4";

/// LLM provider backed by the ZhipuAI Chat Completions API.
#[derive(Debug, Clone)]
pub struct ZhipuProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl ZhipuProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            client: Client::new(),
        }
    }

    /// Create with the default ZhipuAI base URL.
    pub fn with_default_url(api_key: String) -> Self {
        Self::new(api_key, DEFAULT_BASE_URL.to_string())
    }

    fn completions_url(&self) -> String {
        format!(
            "{}/chat/completions",
            self.base_url.trim_end_matches('/')
        )
    }

    fn build_body(&self, request: &CompletionRequest) -> ZhipuRequestBody {
        let messages: Vec<ZhipuMessage> = request
            .messages
            .iter()
            .map(|m| ZhipuMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "tool".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        ZhipuRequestBody {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: request.stream,
        }
    }
}

#[derive(Debug, Serialize)]
struct ZhipuRequestBody {
    model: String,
    messages: Vec<ZhipuMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ZhipuMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ZhipuResponse {
    choices: Vec<ZhipuChoice>,
    usage: Option<ZhipuUsage>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct ZhipuChoice {
    message: ZhipuMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ZhipuUsage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

fn map_finish_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("stop") => StopReason::EndTurn,
        Some("length") => StopReason::MaxTokens,
        Some("tool_calls") | Some("function_call") => StopReason::ToolUse,
        _ => StopReason::EndTurn,
    }
}

#[async_trait]
impl LlmProvider for ZhipuProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let body = self.build_body(&request);
        debug!(model = %request.model, "zhipu complete");

        let resp = self
            .client
            .post(self.completions_url())
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("zhipu request failed: {}", e)))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ClawxError::LlmRateLimited {
                retry_after_secs: 30,
            });
        }
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(ClawxError::LlmProvider(format!(
                "zhipu returned {}: {}",
                status, text
            )));
        }

        let api_resp: ZhipuResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("zhipu parse error: {}", e)))?;

        let choice = api_resp
            .choices
            .first()
            .ok_or_else(|| ClawxError::LlmProvider("zhipu returned no choices".to_string()))?;

        let usage = api_resp.usage.unwrap_or(ZhipuUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });

        Ok(LlmResponse {
            content: choice.message.content.clone(),
            stop_reason: map_finish_reason(choice.finish_reason.as_deref()),
            tool_calls: vec![],
            usage: TokenUsage {
                prompt_tokens: usage.prompt_tokens,
                completion_tokens: usage.completion_tokens,
                total_tokens: usage.total_tokens,
            },
            metadata: Some(ProviderMetadata {
                provider: "zhipu".to_string(),
                model_id: api_resp.model,
                extra: None,
            }),
        })
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk>> + Send>>> {
        let mut body = self.build_body(&request);
        body.stream = true;
        debug!(model = %request.model, "zhipu stream");

        let resp = self
            .client
            .post(self.completions_url())
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("zhipu stream request failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(ClawxError::LlmProvider(format!(
                "zhipu stream returned {}: {}",
                status, text
            )));
        }

        // Parse the SSE byte stream from ZhipuAI into LlmStreamChunks
        let byte_stream = resp.bytes_stream();

        let chunk_stream = async_stream::stream! {
            use futures::StreamExt;
            let mut byte_stream = std::pin::pin!(byte_stream);
            let mut buffer = String::new();

            while let Some(bytes_result) = byte_stream.next().await {
                let bytes = match bytes_result {
                    Ok(b) => b,
                    Err(e) => {
                        yield Err(ClawxError::LlmProvider(format!("zhipu stream read error: {}", e)));
                        return;
                    }
                };

                buffer.push_str(&String::from_utf8_lossy(&bytes));

                // Process complete SSE lines
                while let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim().to_string();
                    buffer = buffer[newline_pos + 1..].to_string();

                    if line.is_empty() || !line.starts_with("data: ") {
                        continue;
                    }

                    let data = &line[6..];
                    if data == "[DONE]" {
                        yield Ok(LlmStreamChunk {
                            delta: String::new(),
                            stop_reason: Some(StopReason::EndTurn),
                            usage: None,
                        });
                        return;
                    }

                    // Parse the ZhipuAI chunk JSON
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(choices) = parsed["choices"].as_array() {
                            if let Some(choice) = choices.first() {
                                let content = choice["delta"]["content"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();
                                let finish = choice["finish_reason"].as_str();
                                let stop = match finish {
                                    Some("stop") => Some(StopReason::EndTurn),
                                    _ => None,
                                };
                                if !content.is_empty() || stop.is_some() {
                                    yield Ok(LlmStreamChunk {
                                        delta: content,
                                        stop_reason: stop,
                                        usage: None,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        };

        Ok(Box::pin(chunk_stream))
    }

    async fn test_connection(&self) -> Result<()> {
        let request = CompletionRequest {
            model: "glm-4-flash".to_string(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "ping".to_string(),
                tool_call_id: None,
            }],
            tools: None,
            temperature: None,
            max_tokens: Some(1),
            stream: false,
        };

        self.complete(request).await.map(|_| ())
    }
}
