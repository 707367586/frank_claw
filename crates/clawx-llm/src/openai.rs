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

    pub(crate) fn build_body(&self, req: &CompletionRequest) -> OpenAiRequestBody {
        use clawx_types::llm::ContentBlock;
        let mut messages = Vec::new();
        for m in &req.messages {
            let role = match m.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            };

            if m.role == MessageRole::Tool {
                // Flatten ToolResult blocks into the top-level content string.
                let mut txt = String::new();
                let mut tool_call_id = m.tool_call_id.clone();
                for b in &m.blocks {
                    if let ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        ..
                    } = b
                    {
                        if !txt.is_empty() {
                            txt.push('\n');
                        }
                        txt.push_str(content);
                        tool_call_id.get_or_insert_with(|| tool_use_id.clone());
                    }
                }
                if txt.is_empty() {
                    txt = m.content.clone();
                }
                messages.push(OpenAiMessage {
                    role: role.into(),
                    content: Some(txt),
                    tool_call_id,
                    tool_calls: vec![],
                });
                continue;
            }

            // Assistant messages may carry ToolUse blocks we need to translate.
            let mut calls: Vec<OpenAiToolCall> = Vec::new();
            for b in &m.blocks {
                if let ContentBlock::ToolUse { id, name, input } = b {
                    calls.push(OpenAiToolCall {
                        id: id.clone(),
                        kind: "function".into(),
                        function: OpenAiFunctionCall {
                            name: name.clone(),
                            arguments: input.to_string(),
                        },
                    });
                }
            }
            let content = if m.content.is_empty() && !calls.is_empty() {
                None
            } else {
                Some(m.content.clone())
            };
            messages.push(OpenAiMessage {
                role: role.into(),
                content,
                tool_call_id: None,
                tool_calls: calls,
            });
        }

        let tools = req.tools.as_ref().map(|defs| {
            defs.iter()
                .map(|d| OpenAiTool {
                    kind: "function",
                    function: OpenAiFunctionDef {
                        name: d.name.clone(),
                        description: d.description.clone(),
                        parameters: d.parameters.clone(),
                    },
                })
                .collect()
        });

        OpenAiRequestBody {
            model: req.model.clone(),
            messages,
            temperature: req.temperature,
            max_tokens: req.max_tokens,
            stream: req.stream,
            tools,
        }
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiRequestBody {
    pub(crate) model: String,
    pub(crate) messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Vec<OpenAiTool>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiTool {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str, // always "function"
    pub(crate) function: OpenAiFunctionDef,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiFunctionDef {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiMessage {
    pub(crate) role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct OpenAiToolCall {
    pub(crate) id: String,
    #[serde(rename = "type", default)]
    pub(crate) kind: String,
    pub(crate) function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct OpenAiFunctionCall {
    pub(crate) name: String,
    pub(crate) arguments: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiResponse {
    #[allow(dead_code)]
    pub(crate) id: String,
    pub(crate) model: String,
    pub(crate) choices: Vec<OpenAiChoice>,
    pub(crate) usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiChoice {
    pub(crate) message: OpenAiRespMessage,
    pub(crate) finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiRespMessage {
    #[serde(default)]
    pub(crate) content: Option<String>,
    #[serde(default)]
    pub(crate) tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiUsage {
    pub(crate) prompt_tokens: u32,
    pub(crate) completion_tokens: u32,
    pub(crate) total_tokens: u32,
}

pub(crate) fn to_llm_response(resp: OpenAiResponse, provider: &'static str) -> LlmResponse {
    let choice = resp.choices.into_iter().next();
    let (content, tool_calls, finish) = match choice {
        Some(c) => {
            let calls = c
                .message
                .tool_calls
                .into_iter()
                .map(|tc| ToolCall {
                    id: tc.id,
                    name: tc.function.name,
                    arguments: serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::String(tc.function.arguments)),
                })
                .collect::<Vec<_>>();
            (
                c.message.content.unwrap_or_default(),
                calls,
                c.finish_reason,
            )
        }
        None => (String::new(), vec![], None),
    };
    let stop_reason = match (finish.as_deref(), tool_calls.is_empty()) {
        (Some("tool_calls"), _) => StopReason::ToolUse,
        (_, false) => StopReason::ToolUse,
        (Some("length"), _) => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    };
    let usage = resp
        .usage
        .map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
        })
        .unwrap_or_default();
    LlmResponse {
        content,
        stop_reason,
        tool_calls,
        usage,
        metadata: Some(ProviderMetadata {
            provider: provider.into(),
            model_id: resp.model,
            extra: None,
        }),
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

        Ok(to_llm_response(api_resp, "openai"))
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
mod tool_calls_tests {
    use super::*;
    use clawx_types::llm::{ContentBlock, ToolDefinition};

    fn dummy() -> OpenAiProvider {
        OpenAiProvider::new("sk-test".into(), "http://127.0.0.1".into())
    }

    #[test]
    fn build_body_serializes_tools_array() {
        let req = CompletionRequest {
            model: "gpt-x".into(),
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
            max_tokens: None,
            stream: false,
        };
        let body = dummy().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["tools"][0]["type"], "function");
        assert_eq!(json["tools"][0]["function"]["name"], "fs_mkdir");
    }

    #[test]
    fn build_body_serializes_tool_result_role() {
        let req = CompletionRequest {
            model: "gpt-x".into(),
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
            max_tokens: None,
            stream: false,
        };
        let body = dummy().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["messages"][0]["role"], "tool");
        assert_eq!(json["messages"][0]["tool_call_id"], "call_1");
        assert_eq!(json["messages"][0]["content"], "ok");
    }

    #[test]
    fn parse_response_maps_tool_calls() {
        let raw = serde_json::json!({
            "id": "c1",
            "model": "gpt-x",
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "fs_mkdir",
                                     "arguments": "{\"path\":\"/tmp/x\"}"}
                    }]
                }
            }],
            "usage": {"prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7}
        });
        let resp: OpenAiResponse = serde_json::from_value(raw).unwrap();
        let mapped = to_llm_response(resp, "openai");
        assert_eq!(mapped.tool_calls.len(), 1);
        assert_eq!(mapped.tool_calls[0].name, "fs_mkdir");
        assert_eq!(mapped.stop_reason, StopReason::ToolUse);
    }
}
