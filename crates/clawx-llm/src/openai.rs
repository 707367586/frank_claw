//! OpenAI (GPT) LLM provider implementation.

use std::pin::Pin;

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;
use futures::stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// LLM provider backed by the OpenAI Chat Completions API.
#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl OpenAiProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            client: Client::new(),
        }
    }

    fn completions_url(&self) -> String {
        format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        )
    }

    fn build_body(&self, request: &CompletionRequest) -> OpenAiRequestBody {
        let messages: Vec<OpenAiMessage> = request
            .messages
            .iter()
            .map(|m| OpenAiMessage {
                role: match m.role {
                    MessageRole::System => "system".to_string(),
                    MessageRole::User => "user".to_string(),
                    MessageRole::Assistant => "assistant".to_string(),
                    MessageRole::Tool => "tool".to_string(),
                },
                content: m.content.clone(),
            })
            .collect();

        OpenAiRequestBody {
            model: request.model.clone(),
            messages,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: request.stream,
        }
    }
}

#[derive(Debug, Serialize)]
struct OpenAiRequestBody {
    model: String,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
    usage: Option<OpenAiUsage>,
    model: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OpenAiUsage {
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
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let body = self.build_body(&request);
        debug!(model = %request.model, "openai complete");

        let resp = self
            .client
            .post(self.completions_url())
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("openai request failed: {}", e)))?;

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
                "openai returned {}: {}",
                status, text
            )));
        }

        let api_resp: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("openai parse error: {}", e)))?;

        let choice = api_resp
            .choices
            .first()
            .ok_or_else(|| ClawxError::LlmProvider("openai returned no choices".to_string()))?;

        let usage = api_resp.usage.unwrap_or(OpenAiUsage {
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
                provider: "openai".to_string(),
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
        debug!(model = %request.model, "openai stream");

        let resp = self
            .client
            .post(self.completions_url())
            .header("Authorization", format!("Bearer {}", &self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("openai stream request failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(ClawxError::LlmProvider(format!(
                "openai stream returned {}: {}",
                status, text
            )));
        }

        // Simplified: read full body and emit as one chunk.
        // Production would parse SSE data: lines incrementally.
        let text = resp
            .text()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("openai stream read error: {}", e)))?;

        let chunk = LlmStreamChunk {
            delta: text,
            stop_reason: Some(StopReason::EndTurn),
            usage: None,
        };

        Ok(Box::pin(stream::once(async move { Ok(chunk) })))
    }

    async fn test_connection(&self) -> Result<()> {
        let request = CompletionRequest {
            model: "gpt-4o-mini".to_string(),
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
