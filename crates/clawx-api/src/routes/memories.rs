//! Memory management API handlers.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use clawx_types::ids::{AgentId, MemoryId};
use clawx_types::memory::*;
use clawx_types::pagination::Pagination;
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
        .route("/", get(list_memories).post(store_memory))
        .route("/{id}", get(get_memory).put(update_memory).delete(delete_memory))
        .route("/{id}/pin", post(toggle_pin))
        .route("/search", post(search_memories))
        .route("/stats", get(get_stats))
}

#[derive(Debug, Deserialize)]
struct ListMemoriesQuery {
    #[serde(default)]
    scope: Option<MemoryScope>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    kind: Option<MemoryKind>,
    #[serde(default)]
    keyword: Option<String>,
    #[serde(default)]
    min_importance: Option<f64>,
    #[serde(default)]
    include_archived: Option<bool>,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_per_page")]
    per_page: u64,
}

fn default_page() -> u64 { 1 }
fn default_per_page() -> u64 { 20 }

async fn list_memories(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<ListMemoriesQuery>,
) -> ApiResult<Json<Value>> {
    let agent_id = query
        .agent_id
        .map(|s| s.parse::<AgentId>())
        .transpose()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id"))?;

    let filter = MemoryFilter {
        scope: query.scope,
        agent_id,
        kind: query.kind,
        keyword: query.keyword,
        min_importance: query.min_importance,
        include_archived: query.include_archived.unwrap_or(false),
    };

    let pagination = Pagination {
        page: query.page,
        per_page: query.per_page.min(100),
    };

    let result = state
        .runtime
        .memory
        .list(filter, pagination)
        .await
        .map_err(internal_err)?;

    Ok(Json(json!({
        "items": result.items,
        "total": result.total,
        "page": result.page,
        "per_page": result.per_page,
    })))
}

async fn store_memory(
    State(state): State<Arc<AppState>>,
    Json(entry): Json<MemoryEntry>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let id = state
        .runtime
        .memory
        .store(entry)
        .await
        .map_err(internal_err)?;

    Ok((StatusCode::CREATED, Json(json!({ "id": id }))))
}

async fn get_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<MemoryEntry>> {
    let memory_id: MemoryId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    let entry = state
        .runtime
        .memory
        .get(memory_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "MEMORY_NOT_FOUND", "memory not found"))?;

    Ok(Json(entry))
}

async fn update_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(mut body): Json<MemoryUpdate>,
) -> ApiResult<StatusCode> {
    let memory_id: MemoryId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    body.id = memory_id;

    state
        .runtime
        .memory
        .update(body)
        .await
        .map_err(internal_err)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn delete_memory(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let memory_id: MemoryId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    state
        .runtime
        .memory
        .delete(memory_id)
        .await
        .map_err(internal_err)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
struct TogglePinRequest {
    #[serde(default = "default_true")]
    pinned: bool,
}

fn default_true() -> bool { true }

async fn toggle_pin(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<TogglePinRequest>,
) -> ApiResult<StatusCode> {
    let memory_id: MemoryId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    state
        .runtime
        .memory
        .toggle_pin(memory_id, body.pinned)
        .await
        .map_err(internal_err)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn search_memories(
    State(state): State<Arc<AppState>>,
    Json(query): Json<MemoryQuery>,
) -> ApiResult<Json<Vec<ScoredMemory>>> {
    let results = state
        .runtime
        .memory
        .recall(query)
        .await
        .map_err(internal_err)?;

    Ok(Json(results))
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    #[serde(default)]
    agent_id: Option<String>,
}

async fn get_stats(
    State(state): State<Arc<AppState>>,
    axum::extract::Query(query): axum::extract::Query<StatsQuery>,
) -> ApiResult<Json<MemoryStats>> {
    let agent_id = query
        .agent_id
        .map(|s| s.parse::<AgentId>())
        .transpose()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id"))?;

    let stats = state
        .runtime
        .memory
        .stats(agent_id)
        .await
        .map_err(internal_err)?;

    Ok(Json(stats))
}
