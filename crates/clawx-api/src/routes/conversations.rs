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
    agent_id: String,
}

async fn list_conversations(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ListConversationsQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let convs = conversation_repo::list_conversations(&state.runtime.db.main, &query.agent_id)
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

/// Invoke the LLM via streaming and return SSE events.
async fn stream_agent_response(
    state: Arc<AppState>,
    conversation_id: String,
    _user_input: String,
) -> Sse<std::pin::Pin<Box<dyn futures::Stream<Item = Result<Event, std::convert::Infallible>> + Send>>> {
    use clawx_types::llm::*;

    // Load conversation history for context
    let messages_json = conversation_repo::list_messages(&state.runtime.db.main, &conversation_id)
        .await
        .unwrap_or_default();

    let mut messages: Vec<Message> = Vec::new();

    messages.push(Message {
        role: MessageRole::System,
        content: "You are a helpful assistant.".to_string(),
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
        model: "default".to_string(),
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

            let sse_stream = llm_stream.map(|chunk_result| match chunk_result {
                Ok(chunk) => Ok(Event::default()
                    .event("delta")
                    .data(
                        serde_json::to_string(&json!({
                            "delta": chunk.delta,
                            "stop_reason": chunk.stop_reason,
                        }))
                        .unwrap_or_default(),
                    )),
                Err(e) => Ok(Event::default()
                    .event("error")
                    .data(json!({"error": e.to_string()}).to_string())),
            });

            let done_event = stream::once(async move {
                // Store the assistant response placeholder
                let _ = conversation_repo::add_message(
                    &state_clone.runtime.db.main,
                    &conv_id,
                    "assistant",
                    "[streamed response]",
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
