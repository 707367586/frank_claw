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
use reqwest::{Client, StatusCode};
use tracing::debug;

use crate::openai::{to_llm_response, OpenAiRequestBody, OpenAiResponse};

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
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    fn build_body(&self, request: &CompletionRequest) -> OpenAiRequestBody {
        // ZhipuAI is OpenAI-compatible on the wire; delegate to the shared
        // OpenAI body builder so tool_calls / tool_result round-trip correctly.
        crate::openai::OpenAiProvider::new(self.api_key.clone(), self.base_url.clone())
            .build_body(request)
    }
}

/// Classify a non-success HTTP response into the appropriate `ClawxError`.
fn classify_http_error(status: StatusCode, body: String) -> ClawxError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => ClawxError::Unauthorized {
            reason: format!("zhipu auth failed ({}): {}", status, body),
        },
        StatusCode::TOO_MANY_REQUESTS => ClawxError::LlmRateLimited {
            retry_after_secs: 30,
        },
        StatusCode::BAD_REQUEST => {
            ClawxError::Validation(format!("zhipu rejected request: {}", body))
        }
        _ => ClawxError::LlmProvider(format!("zhipu returned {}: {}", status, body)),
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
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(classify_http_error(status, text));
        }

        let api_resp: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("zhipu parse error: {}", e)))?;

        Ok(to_llm_response(api_resp, "zhipu"))
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
            return Err(classify_http_error(status, text));
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
                                let stop = finish.map(|r| match r {
                                    "stop" => StopReason::EndTurn,
                                    "length" => StopReason::MaxTokens,
                                    "tool_calls" | "function_call" => StopReason::ToolUse,
                                    "sensitive" => StopReason::StopSequence,
                                    _ => StopReason::EndTurn,
                                });
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
mod tests {
    use super::*;

    #[test]
    fn completions_url_strips_trailing_slash() {
        let p = ZhipuProvider::new("k".into(), "https://example.com/api/".into());
        assert_eq!(
            p.completions_url(),
            "https://example.com/api/chat/completions"
        );
    }

    #[test]
    fn classify_http_error_maps_auth_to_unauthorized() {
        let err = classify_http_error(StatusCode::UNAUTHORIZED, "bad key".into());
        match err {
            ClawxError::Unauthorized { reason } => assert!(reason.contains("bad key")),
            other => panic!("expected Unauthorized, got {:?}", other),
        }

        let err = classify_http_error(StatusCode::FORBIDDEN, "denied".into());
        assert!(matches!(err, ClawxError::Unauthorized { .. }));
    }

    #[test]
    fn classify_http_error_maps_429_to_rate_limited() {
        let err = classify_http_error(StatusCode::TOO_MANY_REQUESTS, "slow down".into());
        assert!(matches!(
            err,
            ClawxError::LlmRateLimited {
                retry_after_secs: 30
            }
        ));
    }

    #[test]
    fn classify_http_error_maps_400_to_validation() {
        let err = classify_http_error(StatusCode::BAD_REQUEST, "invalid model".into());
        match err {
            ClawxError::Validation(msg) => assert!(msg.contains("invalid model")),
            other => panic!("expected Validation, got {:?}", other),
        }
    }

    #[test]
    fn classify_http_error_maps_other_to_llm_provider() {
        let err = classify_http_error(StatusCode::INTERNAL_SERVER_ERROR, "boom".into());
        match err {
            ClawxError::LlmProvider(msg) => {
                assert!(msg.contains("500"));
                assert!(msg.contains("boom"));
            }
            other => panic!("expected LlmProvider, got {:?}", other),
        }
    }
}

#[cfg(test)]
mod tool_calls_tests {
    use super::*;
    use clawx_types::llm::{ContentBlock, ToolDefinition};

    fn dummy() -> ZhipuProvider {
        ZhipuProvider::new("test-key".into(), "http://127.0.0.1".into())
    }

    #[test]
    fn build_body_serializes_tools_array() {
        let req = CompletionRequest {
            model: "glm-4".into(),
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
            model: "glm-4".into(),
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
            "model": "glm-4",
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
        let mapped = to_llm_response(resp, "zhipu");
        assert_eq!(mapped.tool_calls.len(), 1);
        assert_eq!(mapped.tool_calls[0].name, "fs_mkdir");
        assert_eq!(mapped.stop_reason, StopReason::ToolUse);
    }
}
