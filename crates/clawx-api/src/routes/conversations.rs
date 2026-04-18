//! Conversation and message management API handlers.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use clawx_runtime::conversation_repo;
use futures::stream;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::AppState;

type ApiResult<T> = std::result::Result<T, (StatusCode, Json<Value>)>;

fn err_response(status: StatusCode, code: &str, message: &str) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
}

fn internal_err(msg: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    err_response(
        StatusCode::INTERNAL_SERVER_ERROR,
        "INTERNAL_ERROR",
        &msg.to_string(),
    )
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_conversations).post(create_conversation))
        .route("/{id}", get(get_conversation).delete(delete_conversation))
        .route("/{id}/messages", get(list_messages).post(add_message))
}

#[derive(Debug, Deserialize)]
struct CreateConversationRequest {
    agent_id: String,
    #[serde(default)]
    title: Option<String>,
}

async fn create_conversation(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateConversationRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let _: uuid::Uuid = body
        .agent_id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "agent_id must be a valid UUID"))?;

    let conv_id = conversation_repo::create_conversation(
        &state.runtime.db.main,
        &body.agent_id,
        body.title.as_deref(),
    )
    .await
    .map_err(internal_err)?;

    let conv = conversation_repo::get_conversation(&state.runtime.db.main, &conv_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| internal_err("conversation not found after create"))?;

    Ok((StatusCode::CREATED, Json(conv)))
}

#[derive(Debug, Deserialize)]
struct ListConversationsQuery {
    #[serde(default)]
    agent_id: Option<String>,
}

async fn list_conversations(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ListConversationsQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let convs = conversation_repo::list_conversations(
        &state.runtime.db.main,
        query.agent_id.as_deref(),
    )
    .await
    .map_err(internal_err)?;
    Ok(Json(convs))
}

async fn get_conversation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let conv = conversation_repo::get_conversation(&state.runtime.db.main, &id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| {
            err_response(
                StatusCode::NOT_FOUND,
                "CONVERSATION_NOT_FOUND",
                "conversation not found",
            )
        })?;

    Ok(Json(conv))
}

async fn delete_conversation(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    conversation_repo::delete_conversation(&state.runtime.db.main, &id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => err_response(
                StatusCode::NOT_FOUND,
                "CONVERSATION_NOT_FOUND",
                "conversation not found",
            ),
            _ => internal_err(e),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_messages(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Value>>> {
    let msgs = conversation_repo::list_messages(&state.runtime.db.main, &id)
        .await
        .map_err(internal_err)?;
    Ok(Json(msgs))
}

#[derive(Debug, Deserialize)]
struct AddMessageRequest {
    role: String,
    content: String,
    /// When true and role=user, invoke LLM and return SSE stream.
    #[serde(default)]
    stream: bool,
}

async fn add_message(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AddMessageRequest>,
) -> Result<axum::response::Response, (StatusCode, Json<Value>)> {
    // Verify conversation exists
    conversation_repo::get_conversation(&state.runtime.db.main, &id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| {
            err_response(
                StatusCode::NOT_FOUND,
                "CONVERSATION_NOT_FOUND",
                "conversation not found",
            )
        })?;

    // Store the user message
    let msg_id = conversation_repo::add_message(
        &state.runtime.db.main,
        &id,
        &body.role,
        &body.content,
    )
    .await
    .map_err(internal_err)?;

    // If stream=true and role=user, invoke LLM and return SSE
    if body.stream && body.role == "user" {
        return Ok(stream_agent_response(state, id, body.content).await.into_response());
    }

    // Non-streaming: return the stored message
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": msg_id,
            "conversation_id": id,
            "role": body.role,
            "content": body.content,
        })),
    )
        .into_response())
}

/// Create an SSE `Event` for an execution step.
/// Called by the Agent Loop when TaskExecutor emits step progress.
#[allow(dead_code)]
pub fn execution_step_event(step: &clawx_types::event::SseExecutionStep) -> Event {
    Event::default()
        .event("execution_step")
        .data(serde_json::to_string(step).unwrap_or_default())
}

/// Create an SSE `Event` for a confirmation-required prompt.
/// Called by the Agent Loop when TaskExecutor needs user confirmation.
#[allow(dead_code)]
pub fn confirmation_required_event(
    step_no: u32,
    tool_name: &str,
    risk_level: &str,
    reason: &str,
) -> Event {
    let payload = clawx_types::event::SseConfirmationRequired {
        step_no,
        tool_name: tool_name.to_string(),
        risk_level: risk_level.to_string(),
        reason: reason.to_string(),
    };
    Event::default()
        .event("confirmation_required")
        .data(serde_json::to_string(&payload).unwrap_or_default())
}

/// Invoke the LLM via streaming and return SSE events.
async fn stream_agent_response(
    state: Arc<AppState>,
    conversation_id: String,
    _user_input: String,
) -> Sse<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, std::convert::Infallible>> + Send>>> {
    use clawx_types::llm::*;

    // Load conversation → agent → provider to get system prompt and model name
    let conv_json = conversation_repo::get_conversation(&state.runtime.db.main, &conversation_id)
        .await
        .ok()
        .flatten();

    let (system_prompt, model_name) = {
        let mut prompt = "You are a helpful assistant.".to_string();
        let mut model = "default".to_string();

        if let Some(ref conv) = conv_json {
            if let Some(agent_id_str) = conv["agent_id"].as_str() {
                if let Ok(aid) = agent_id_str.parse::<clawx_types::ids::AgentId>() {
                    if let Ok(Some(agent)) = clawx_runtime::agent_repo::get_agent(&state.runtime.db.main, &aid).await {
                        if let Some(sp) = &agent.system_prompt {
                            prompt = sp.clone();
                        }
                        // Look up the agent's model provider for the actual model name
                        if let Ok(Some(provider)) = clawx_runtime::model_repo::get_provider(
                            &state.runtime.db.main,
                            &agent.model_id,
                        ).await {
                            model = provider.model_name;
                        }
                    }
                }
            }
        }
        (prompt, model)
    };

    // Load conversation history for context
    let messages_json = conversation_repo::list_messages(&state.runtime.db.main, &conversation_id)
        .await
        .unwrap_or_default();

    let mut messages: Vec<Message> = Vec::new();

    messages.push(Message {
        role: MessageRole::System,
        content: system_prompt,
        tool_call_id: None,
    });

    for msg in &messages_json {
        let role = match msg["role"].as_str().unwrap_or("user") {
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => MessageRole::User,
        };
        messages.push(Message {
            role,
            content: msg["content"].as_str().unwrap_or("").to_string(),
            tool_call_id: None,
        });
    }

    let request = CompletionRequest {
        model: model_name,
        messages,
        tools: None,
        temperature: None,
        max_tokens: Some(4096),
        stream: true,
    };

    match state.runtime.llm.stream(request).await {
        Ok(llm_stream) => {
            let state_clone = state.clone();
            let conv_id = conversation_id.clone();

            // Accumulate deltas as the stream is consumed so we can persist
            // the full assistant response on the terminal `done` event.
            let accumulator: Arc<tokio::sync::Mutex<String>> =
                Arc::new(tokio::sync::Mutex::new(String::new()));

            let acc_for_stream = accumulator.clone();
            let sse_stream = llm_stream.then(move |chunk_result| {
                let acc = acc_for_stream.clone();
                async move {
                    match chunk_result {
                        Ok(chunk) => {
                            if !chunk.delta.is_empty() {
                                acc.lock().await.push_str(&chunk.delta);
                            }
                            Ok::<Event, std::convert::Infallible>(Event::default()
                                .event("delta")
                                .data(
                                    serde_json::to_string(&json!({
                                        "delta": chunk.delta,
                                        "stop_reason": chunk.stop_reason,
                                    }))
                                    .unwrap_or_default(),
                                ))
                        }
                        Err(e) => Ok::<Event, std::convert::Infallible>(Event::default()
                            .event("error")
                            .data(json!({"error": e.to_string()}).to_string())),
                    }
                }
            });

            let acc_for_done = accumulator.clone();
            let done_event = stream::once(async move {
                // Persist the accumulated assistant response.
                let final_content = acc_for_done.lock().await.clone();
                let persisted = if final_content.is_empty() {
                    "(empty response)"
                } else {
                    final_content.as_str()
                };
                let _ = conversation_repo::add_message(
                    &state_clone.runtime.db.main,
                    &conv_id,
                    "assistant",
                    persisted,
                )
                .await;

                Ok::<_, std::convert::Infallible>(
                    Event::default()
                        .event("done")
                        .data(json!({"status": "complete"}).to_string()),
                )
            });

            Sse::new(Box::pin(sse_stream.chain(done_event))
                as std::pin::Pin<
                    Box<
                        dyn futures::Stream<Item = Result<Event, std::convert::Infallible>> + Send,
                    >,
                >)
        }
        Err(e) => {
            let error_stream = stream::once(async move {
                Ok::<_, std::convert::Infallible>(
                    Event::default()
                        .event("error")
                        .data(json!({"error": e.to_string()}).to_string()),
                )
            });
            Sse::new(Box::pin(error_stream)
                as std::pin::Pin<
                    Box<
                        dyn futures::Stream<Item = Result<Event, std::convert::Infallible>> + Send,
                    >,
                >)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::event::{SseConfirmationRequired, SseExecutionStep};

    #[test]
    fn test_execution_step_event_format() {
        let step = SseExecutionStep {
            step_no: 1,
            action: "search".to_string(),
            tool: "web_search".to_string(),
            evidence: "found 3 results".to_string(),
            result_summary: "Top result is Wikipedia".to_string(),
        };

        let event = execution_step_event(&step);
        // Event::into_parts is not public, so we verify via Debug representation
        let debug = format!("{:?}", event);
        assert!(debug.contains("execution_step"), "event type must be execution_step");
        assert!(debug.contains("web_search"), "data must contain tool name");
        assert!(debug.contains("search"), "data must contain action");
        assert!(debug.contains("found 3 results"), "data must contain evidence");

        // Also verify the JSON data round-trips correctly
        let json_str = serde_json::to_string(&step).unwrap();
        let parsed: SseExecutionStep = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed, step);
    }

    #[test]
    fn test_confirmation_required_event_format() {
        let event = confirmation_required_event(2, "delete_file", "high", "destructive operation");
        let debug = format!("{:?}", event);
        assert!(debug.contains("confirmation_required"), "event type must be confirmation_required");
        assert!(debug.contains("delete_file"), "data must contain tool_name");
        assert!(debug.contains("high"), "data must contain risk_level");
        assert!(debug.contains("destructive operation"), "data must contain reason");

        // Verify the struct serializes correctly
        let payload = SseConfirmationRequired {
            step_no: 2,
            tool_name: "delete_file".to_string(),
            risk_level: "high".to_string(),
            reason: "destructive operation".to_string(),
        };
        let json_str = serde_json::to_string(&payload).unwrap();
        let parsed: SseConfirmationRequired = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed, payload);
        assert_eq!(parsed.step_no, 2);
    }
}
