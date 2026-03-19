//! Knowledge base management API handlers.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use clawx_types::ids::KnowledgeSourceId;
use clawx_types::knowledge::{SearchQuery, SearchResult};
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
        .route("/sources", get(list_sources).post(add_source))
        .route("/sources/{id}", get(get_source).delete(remove_source))
        .route("/search", axum::routing::post(search))
}

async fn list_sources(
    State(_state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<Value>>> {
    // KnowledgeService trait doesn't have list_sources — return empty for now
    // This will be expanded when the trait gains a list method
    Ok(Json(vec![]))
}

#[derive(Debug, Deserialize)]
struct AddSourceRequest {
    path: String,
    #[serde(default)]
    agent_id: Option<String>,
}

async fn add_source(
    State(state): State<Arc<AppState>>,
    Json(body): Json<AddSourceRequest>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let agent_id = body
        .agent_id
        .map(|s| s.parse())
        .transpose()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id"))?;

    // Validate path before passing to knowledge service
    let path_decision = state
        .runtime
        .security
        .check_path(&body.path)
        .await
        .map_err(internal_err)?;
    if let clawx_types::security::SecurityDecision::Deny { reason } = path_decision {
        return Err(err_response(
            StatusCode::FORBIDDEN,
            "PATH_DENIED",
            &reason,
        ));
    }

    let source_id = state
        .runtime
        .knowledge
        .add_source(body.path.clone(), agent_id)
        .await
        .map_err(internal_err)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "id": source_id,
            "path": body.path,
        })),
    ))
}

async fn get_source(
    State(_state): State<Arc<AppState>>,
    Path(_id): Path<String>,
) -> ApiResult<Json<Value>> {
    // KnowledgeService trait doesn't have get_source — return 501 for now
    Err(err_response(
        StatusCode::NOT_IMPLEMENTED,
        "NOT_IMPLEMENTED",
        "get source not yet implemented",
    ))
}

async fn remove_source(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let source_id: KnowledgeSourceId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    state
        .runtime
        .knowledge
        .remove_source(source_id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "SOURCE_NOT_FOUND", "source not found")
            }
            _ => internal_err(e),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn search(
    State(state): State<Arc<AppState>>,
    Json(query): Json<SearchQuery>,
) -> ApiResult<Json<Vec<SearchResult>>> {
    let results = state
        .runtime
        .knowledge
        .search(query)
        .await
        .map_err(internal_err)?;

    Ok(Json(results))
}
