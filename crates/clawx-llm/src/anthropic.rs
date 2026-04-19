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
        let mut system: Option<String> = None;
        let mut messages: Vec<AnthropicMessage> = Vec::new();

        for msg in &request.messages {
            match msg.role {
                MessageRole::System => system = Some(msg.content.clone()),
                MessageRole::User | MessageRole::Assistant | MessageRole::Tool => {
                    let role = match msg.role {
                        MessageRole::Assistant => "assistant",
                        // Tool-result messages ride on `user` per Anthropic's schema.
                        _ => "user",
                    };
                    let content = if msg.blocks.is_empty() {
                        AnthropicContentField::Text(msg.content.clone())
                    } else {
                        AnthropicContentField::Blocks(
                            msg.blocks.iter().map(to_anthropic_block).collect(),
                        )
                    };
                    messages.push(AnthropicMessage {
                        role: role.to_string(),
                        content,
                    });
                }
            }
        }

        let tools = request.tools.as_ref().map(|defs| {
            defs.iter()
                .map(|d| AnthropicTool {
                    name: d.name.clone(),
                    description: d.description.clone(),
                    input_schema: d.parameters.clone(),
                })
                .collect()
        });

        AnthropicRequestBody {
            model: request.model.clone(),
            max_tokens: request.max_tokens.unwrap_or(4096),
            system,
            messages,
            temperature: request.temperature,
            stream: request.stream,
            tools,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContentField,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicContentField {
    Text(String),
    Blocks(Vec<AnthropicBlock>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

fn to_anthropic_block(b: &clawx_types::llm::ContentBlock) -> AnthropicBlock {
    use clawx_types::llm::ContentBlock::*;
    match b {
        Text { text } => AnthropicBlock::Text { text: text.clone() },
        ToolUse { id, name, input } => AnthropicBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        ToolResult {
            tool_use_id,
            content,
            is_error,
        } => AnthropicBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: content.clone(),
            is_error: *is_error,
        },
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    #[allow(dead_code)]
    id: String,
    content: Vec<AnthropicBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
    model: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

fn to_llm_response(raw: AnthropicResponse) -> LlmResponse {
    let mut text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    for block in raw.content {
        match block {
            AnthropicBlock::Text { text: t } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&t);
            }
            AnthropicBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id,
                    name,
                    arguments: input,
                });
            }
            AnthropicBlock::ToolResult { .. } => {
                // Providers never emit tool_result back to us; ignore.
            }
        }
    }
    LlmResponse {
        content: text,
        stop_reason: map_stop_reason(raw.stop_reason.as_deref()),
        tool_calls,
        usage: TokenUsage {
            prompt_tokens: raw.usage.input_tokens,
            completion_tokens: raw.usage.output_tokens,
            total_tokens: raw.usage.input_tokens + raw.usage.output_tokens,
        },
        metadata: Some(ProviderMetadata {
            provider: "anthropic".into(),
            model_id: raw.model,
            extra: None,
        }),
    }
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

        let raw: AnthropicResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("anthropic parse error: {}", e)))?;

        Ok(to_llm_response(raw))
    }

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk>> + Send>>> {
        if request.tools.is_some() {
            return Err(ClawxError::LlmProvider(
                "streaming with tools is not supported in Phase 1".into(),
            ));
        }
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
            .map_err(|e| {
                ClawxError::LlmProvider(format!("anthropic stream request failed: {}", e))
            })?;

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

#[cfg(test)]
mod tool_use_tests {
    use super::*;
    use clawx_types::llm::{ContentBlock, ToolDefinition};

    fn dummy_provider() -> AnthropicProvider {
        AnthropicProvider::new("sk-test".into(), "http://127.0.0.1".into())
    }

    #[test]
    fn build_body_serializes_tools() {
        let req = CompletionRequest {
            model: "claude-sonnet-4-6".into(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "hi".into(),
                blocks: vec![],
                tool_call_id: None,
            }],
            tools: Some(vec![ToolDefinition {
                name: "fs_mkdir".into(),
                description: "Create a directory".into(),
                parameters: serde_json::json!({"type":"object"}),
            }]),
            temperature: None,
            max_tokens: Some(256),
            stream: false,
        };
        let body = dummy_provider().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["tools"][0]["name"], "fs_mkdir");
        assert_eq!(json["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn build_body_serializes_tool_result_message_as_user_block() {
        let req = CompletionRequest {
            model: "claude-sonnet-4-6".into(),
            messages: vec![Message {
                role: MessageRole::Tool,
                content: String::new(),
                blocks: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "ok".into(),
                    is_error: false,
                }],
                tool_call_id: Some("call_1".into()),
            }],
            tools: None,
            temperature: None,
            max_tokens: Some(256),
            stream: false,
        };
        let body = dummy_provider().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"][0]["type"], "tool_result");
        assert_eq!(json["messages"][0]["content"][0]["tool_use_id"], "call_1");
        assert!(
            json["messages"][0]["content"][0].get("is_error").is_none(),
            "is_error: false MUST be omitted on the wire",
        );
    }

    #[test]
    fn parse_response_extracts_tool_use_blocks() {
        let raw = serde_json::json!({
            "id": "msg_1",
            "model": "claude-sonnet-4-6",
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 5, "output_tokens": 7},
            "content": [
                {"type": "text", "text": "I'll create it."},
                {"type": "tool_use", "id": "call_1", "name": "fs_mkdir",
                 "input": {"path": "/tmp/x"}}
            ]
        });
        let resp: AnthropicResponse = serde_json::from_value(raw).unwrap();
        let mapped = to_llm_response(resp);
        assert_eq!(mapped.tool_calls.len(), 1);
        assert_eq!(mapped.tool_calls[0].name, "fs_mkdir");
        assert_eq!(mapped.stop_reason, StopReason::ToolUse);
        assert!(mapped.content.contains("I'll create it."));
    }
}
