//! Vault (workspace version management) API handlers.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use clawx_types::ids::SnapshotId;
use clawx_types::vault::{DiffPreview, VaultSnapshot};
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
        .route("/", get(list_snapshots).post(create_snapshot))
        .route("/{id}", get(get_snapshot))
        .route("/{id}/diff", get(diff_preview))
        .route("/{id}/rollback", axum::routing::post(rollback))
}

async fn list_snapshots(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<VaultSnapshot>>> {
    let snapshots = state
        .runtime
        .vault
        .list_snapshots()
        .await
        .map_err(internal_err)?;

    Ok(Json(snapshots))
}

#[derive(Debug, Deserialize)]
struct CreateSnapshotRequest {
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    task_id: Option<String>,
    #[serde(default)]
    description: Option<String>,
}

async fn create_snapshot(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateSnapshotRequest>,
) -> ApiResult<(StatusCode, Json<VaultSnapshot>)> {
    let agent_id = body
        .agent_id
        .map(|s| s.parse())
        .transpose()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id"))?;

    let task_id = body
        .task_id
        .map(|s| s.parse())
        .transpose()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_TASK_ID", "invalid task_id"))?;

    let snapshot = state
        .runtime
        .vault
        .create_snapshot(agent_id, task_id, body.description)
        .await
        .map_err(internal_err)?;

    Ok((StatusCode::CREATED, Json(snapshot)))
}

async fn get_snapshot(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<VaultSnapshot>> {
    let snapshot_id: SnapshotId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    // Use diff_preview to get the snapshot (it includes the snapshot data)
    let preview = state
        .runtime
        .vault
        .diff_preview(snapshot_id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "SNAPSHOT_NOT_FOUND", "snapshot not found")
            }
            _ => internal_err(e),
        })?;

    Ok(Json(preview.snapshot))
}

async fn diff_preview(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<DiffPreview>> {
    let snapshot_id: SnapshotId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    let preview = state
        .runtime
        .vault
        .diff_preview(snapshot_id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "SNAPSHOT_NOT_FOUND", "snapshot not found")
            }
            _ => internal_err(e),
        })?;

    Ok(Json(preview))
}

async fn rollback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let snapshot_id: SnapshotId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    state
        .runtime
        .vault
        .rollback(snapshot_id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "SNAPSHOT_NOT_FOUND", "snapshot not found")
            }
            _ => internal_err(e),
        })?;

    Ok(StatusCode::NO_CONTENT)
}
