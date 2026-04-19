//! Anthropic (Claude) LLM provider implementation.

use std::pin::Pin;

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;
use futures::stream;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::debug;

/// LLM provider backed by the Anthropic Messages API.
#[derive(Debug, Clone)]
pub struct AnthropicProvider {
    api_key: String,
    base_url: String,
    client: Client,
}

impl AnthropicProvider {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            api_key,
            base_url,
            client: Client::new(),
        }
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.base_url.trim_end_matches('/'))
    }

    fn build_body(&self, request: &CompletionRequest) -> AnthropicRequestBody {
        // Separate system message from conversation messages
        let mut system: Option<String> = None;
        let mut messages = Vec::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => {
                    system = Some(msg.content.clone());
                }
                _ => {
                    messages.push(AnthropicMessage {
                        role: match msg.role {
                            MessageRole::User => "user".to_string(),
                            MessageRole::Assistant => "assistant".to_string(),
                            MessageRole::Tool => "user".to_string(), // Anthropic uses user for tool results
                            MessageRole::System => unreachable!(),
                        },
                        content: msg.content.clone(),
                    });
                }
            }
        }

        AnthropicRequestBody {
            model: request.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(4096),
            system,
            messages,
            temperature: request.temperature,
            stream: request.stream,
        }
    }
}

#[derive(Debug, Serialize)]
struct AnthropicRequestBody {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    content: Vec<AnthropicContent>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
    model: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicContent {
    #[serde(rename = "type")]
    #[allow(dead_code)]
    content_type: String,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

fn map_stop_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("end_turn") => StopReason::EndTurn,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("tool_use") => StopReason::ToolUse,
        Some("stop_sequence") => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    }
}

#[async_trait]
impl LlmProvider for AnthropicProvider {
    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse> {
        let body = self.build_body(&request);
        debug!(model = %request.model, "anthropic complete");

        let resp = self
            .client
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("anthropic request failed: {}", e)))?;

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
                "anthropic returned {}: {}",
                status, text
            )));
        }

        let api_resp: AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("anthropic parse error: {}", e)))?;

        let content = api_resp
            .content
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("");

        Ok(LlmResponse {
            content,
            stop_reason: map_stop_reason(api_resp.stop_reason.as_deref()),
            tool_calls: vec![],
            usage: TokenUsage {
                prompt_tokens: api_resp.usage.input_tokens,
                completion_tokens: api_resp.usage.output_tokens,
                total_tokens: api_resp.usage.input_tokens + api_resp.usage.output_tokens,
            },
            metadata: Some(ProviderMetadata {
                provider: "anthropic".to_string(),
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
        debug!(model = %request.model, "anthropic stream");

        let resp = self
            .client
            .post(self.messages_url())
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("anthropic stream request failed: {}", e)))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(ClawxError::LlmProvider(format!(
                "anthropic stream returned {}: {}",
                status, text
            )));
        }

        // For now, read the full response and return as a single chunk.
        // A production implementation would parse SSE events incrementally.
        let text = resp
            .text()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("anthropic stream read error: {}", e)))?;

        let chunk = LlmStreamChunk {
            delta: text,
            stop_reason: Some(StopReason::EndTurn),
            usage: None,
        };

        Ok(Box::pin(stream::once(async move { Ok(chunk) })))
    }

    async fn test_connection(&self) -> Result<()> {
        let request = CompletionRequest {
            model: "claude-3-haiku-20240307".to_string(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "ping".to_string(),
                blocks: vec![],
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
