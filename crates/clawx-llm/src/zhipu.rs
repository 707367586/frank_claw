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
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    fn build_body(&self, request: &CompletionRequest) -> ZhipuRequestBody {
        let messages: Vec<ZhipuMessage> = request.messages.iter().map(message_to_zhipu).collect();

        let tools = request.tools.as_ref().map(|defs| {
            defs.iter()
                .map(|t| ZhipuTool {
                    tool_type: "function".to_string(),
                    function: ZhipuToolFunction {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.parameters.clone(),
                    },
                })
                .collect()
        });

        ZhipuRequestBody {
            model: request.model.clone(),
            messages,
            tools,
            temperature: request.temperature,
            max_tokens: request.max_tokens,
            stream: request.stream,
        }
    }
}

fn message_to_zhipu(m: &Message) -> ZhipuMessage {
    ZhipuMessage {
        role: match m.role {
            MessageRole::System => "system".to_string(),
            MessageRole::User => "user".to_string(),
            MessageRole::Assistant => "assistant".to_string(),
            MessageRole::Tool => "tool".to_string(),
        },
        content: Some(m.content.clone()),
        tool_call_id: m.tool_call_id.clone(),
        tool_calls: None,
    }
}

#[derive(Debug, Serialize)]
struct ZhipuRequestBody {
    model: String,
    messages: Vec<ZhipuMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ZhipuTool>>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ZhipuToolCall>>,
}

#[derive(Debug, Serialize)]
struct ZhipuTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: ZhipuToolFunction,
}

#[derive(Debug, Serialize)]
struct ZhipuToolFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct ZhipuToolCall {
    id: String,
    #[serde(default, rename = "type")]
    call_type: Option<String>,
    function: ZhipuToolCallFunction,
}

#[derive(Debug, Serialize, Deserialize)]
struct ZhipuToolCallFunction {
    name: String,
    /// ZhipuAI returns arguments as a JSON-encoded string (OpenAI-compatible).
    arguments: String,
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
        Some("sensitive") => StopReason::StopSequence,
        _ => StopReason::EndTurn,
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

fn parse_tool_calls(raw: &[ZhipuToolCall]) -> Result<Vec<ToolCall>> {
    raw.iter()
        .map(|tc| {
            let arguments: serde_json::Value = if tc.function.arguments.is_empty() {
                serde_json::Value::Object(Default::default())
            } else {
                serde_json::from_str(&tc.function.arguments).map_err(|e| {
                    ClawxError::LlmProvider(format!(
                        "zhipu tool_call arguments are not valid JSON: {}",
                        e
                    ))
                })?
            };
            Ok(ToolCall {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                arguments,
            })
        })
        .collect()
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

        let api_resp: ZhipuResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::LlmProvider(format!("zhipu parse error: {}", e)))?;

        let choice = api_resp
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ClawxError::LlmProvider("zhipu returned no choices".to_string()))?;

        let usage = api_resp.usage.unwrap_or(ZhipuUsage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });

        let tool_calls = choice
            .message
            .tool_calls
            .as_deref()
            .map(parse_tool_calls)
            .transpose()?
            .unwrap_or_default();

        Ok(LlmResponse {
            content: choice.message.content.unwrap_or_default(),
            stop_reason: map_finish_reason(choice.finish_reason.as_deref()),
            tool_calls,
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
                                let stop = finish.map(|r| map_finish_reason(Some(r)));
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
    use serde_json::json;

    fn provider() -> ZhipuProvider {
        ZhipuProvider::with_default_url("test-key".to_string())
    }

    fn base_request() -> CompletionRequest {
        CompletionRequest {
            model: "glm-4-flash".to_string(),
            messages: vec![
                Message {
                    role: MessageRole::System,
                    content: "you are helpful".to_string(),
                    blocks: vec![],
                    tool_call_id: None,
                },
                Message {
                    role: MessageRole::User,
                    content: "hello".to_string(),
                    blocks: vec![],
                    tool_call_id: None,
                },
            ],
            tools: None,
            temperature: Some(0.7),
            max_tokens: Some(256),
            stream: false,
        }
    }

    #[test]
    fn completions_url_strips_trailing_slash() {
        let p = ZhipuProvider::new("k".into(), "https://example.com/api/".into());
        assert_eq!(p.completions_url(), "https://example.com/api/chat/completions");
    }

    #[test]
    fn build_body_without_tools_omits_tools_field() {
        let body = provider().build_body(&base_request());
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["model"], "glm-4-flash");
        assert_eq!(json["messages"].as_array().unwrap().len(), 2);
        assert_eq!(json["messages"][0]["role"], "system");
        assert_eq!(json["messages"][1]["role"], "user");
        assert_eq!(json["temperature"], 0.7);
        assert_eq!(json["max_tokens"], 256);
        assert!(json.get("tools").is_none(), "tools must be omitted when None");
        assert!(json.get("stream").is_none(), "stream must be omitted when false");
    }

    #[test]
    fn build_body_with_tools_serializes_function_schema() {
        let mut req = base_request();
        req.tools = Some(vec![ToolDefinition {
            name: "get_weather".to_string(),
            description: "look up weather".to_string(),
            parameters: json!({
                "type": "object",
                "properties": { "city": { "type": "string" } },
                "required": ["city"]
            }),
        }]);

        let body = provider().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        let tools = json["tools"].as_array().expect("tools present");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "get_weather");
        assert_eq!(tools[0]["function"]["description"], "look up weather");
        assert_eq!(
            tools[0]["function"]["parameters"]["required"][0],
            "city"
        );
    }

    #[test]
    fn build_body_forwards_tool_result_messages() {
        let mut req = base_request();
        req.messages.push(Message {
            role: MessageRole::Tool,
            content: "{\"temp\":22}".to_string(),
            blocks: vec![],
            tool_call_id: Some("call_1".to_string()),
        });

        let body = provider().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        let last = &json["messages"][2];
        assert_eq!(last["role"], "tool");
        assert_eq!(last["content"], "{\"temp\":22}");
        assert_eq!(last["tool_call_id"], "call_1");
    }

    #[test]
    fn map_finish_reason_covers_known_values() {
        assert_eq!(map_finish_reason(Some("stop")), StopReason::EndTurn);
        assert_eq!(map_finish_reason(Some("length")), StopReason::MaxTokens);
        assert_eq!(map_finish_reason(Some("tool_calls")), StopReason::ToolUse);
        assert_eq!(
            map_finish_reason(Some("function_call")),
            StopReason::ToolUse
        );
        assert_eq!(
            map_finish_reason(Some("sensitive")),
            StopReason::StopSequence
        );
        assert_eq!(map_finish_reason(None), StopReason::EndTurn);
        assert_eq!(map_finish_reason(Some("unknown")), StopReason::EndTurn);
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
            ClawxError::LlmRateLimited { retry_after_secs: 30 }
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

    #[test]
    fn parse_tool_calls_decodes_json_arguments() {
        let raw = vec![ZhipuToolCall {
            id: "call_abc".into(),
            call_type: Some("function".into()),
            function: ZhipuToolCallFunction {
                name: "get_weather".into(),
                arguments: "{\"city\":\"Shanghai\"}".into(),
            },
        }];
        let parsed = parse_tool_calls(&raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].id, "call_abc");
        assert_eq!(parsed[0].name, "get_weather");
        assert_eq!(parsed[0].arguments["city"], "Shanghai");
    }

    #[test]
    fn parse_tool_calls_empty_arguments_become_empty_object() {
        let raw = vec![ZhipuToolCall {
            id: "call_x".into(),
            call_type: None,
            function: ZhipuToolCallFunction {
                name: "noop".into(),
                arguments: String::new(),
            },
        }];
        let parsed = parse_tool_calls(&raw).unwrap();
        assert!(parsed[0].arguments.is_object());
        assert_eq!(parsed[0].arguments.as_object().unwrap().len(), 0);
    }

    #[test]
    fn parse_tool_calls_rejects_invalid_json() {
        let raw = vec![ZhipuToolCall {
            id: "call_x".into(),
            call_type: None,
            function: ZhipuToolCallFunction {
                name: "bad".into(),
                arguments: "not json".into(),
            },
        }];
        let err = parse_tool_calls(&raw).unwrap_err();
        assert!(matches!(err, ClawxError::LlmProvider(_)));
    }

    #[test]
    fn zhipu_response_deserializes_with_tool_calls() {
        let payload = json!({
            "model": "glm-4",
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "get_weather",
                            "arguments": "{\"city\":\"Beijing\"}"
                        }
                    }]
                }
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15 }
        });

        let resp: ZhipuResponse = serde_json::from_value(payload).unwrap();
        assert_eq!(resp.model, "glm-4");
        let choice = &resp.choices[0];
        assert_eq!(choice.finish_reason.as_deref(), Some("tool_calls"));
        assert!(choice.message.content.is_none());
        let tcs = choice.message.tool_calls.as_ref().unwrap();
        assert_eq!(tcs.len(), 1);
        assert_eq!(tcs[0].function.name, "get_weather");

        let parsed = parse_tool_calls(tcs).unwrap();
        assert_eq!(parsed[0].arguments["city"], "Beijing");
    }
}
