//! Skills management API handlers (v0.2).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

use clawx_runtime::skill_repo;
use clawx_types::ids::SkillId;
use clawx_types::skill::SkillManifest;

use crate::AppState;

type ApiResult<T> = std::result::Result<T, (StatusCode, Json<Value>)>;

fn err_response(status: StatusCode, code: &str, message: &str) -> (StatusCode, Json<Value>) {
    (status, Json(json!({ "error": { "code": code, "message": message } })))
}

fn internal_err(msg: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    err_response(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", &msg.to_string())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_skills).post(install_skill_handler))
        .route("/{id}", get(get_skill).delete(uninstall_skill))
        .route("/{id}/enable", axum::routing::post(enable_skill))
        .route("/{id}/disable", axum::routing::post(disable_skill))
}

/// Payload for installing a new skill via POST /skills.
#[derive(Deserialize)]
struct InstallSkillPayload {
    manifest: SkillManifest,
    /// Hex-encoded WASM bytes.
    wasm_bytes_hex: String,
    /// Optional hex-encoded Ed25519 signature.
    #[serde(default)]
    signature: Option<String>,
}

async fn install_skill_handler(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<InstallSkillPayload>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let wasm_bytes = hex::decode(&payload.wasm_bytes_hex)
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_WASM", "invalid hex-encoded wasm bytes"))?;

    let skill = skill_repo::install_skill(
        &state.runtime.db.main,
        &payload.manifest,
        &wasm_bytes,
        payload.signature.as_deref(),
    )
    .await
    .map_err(internal_err)?;

    Ok((StatusCode::CREATED, Json(serde_json::to_value(skill).unwrap())))
}

async fn list_skills(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Value>> {
    let skills = skill_repo::list_skills(&state.runtime.db.main)
        .await
        .map_err(internal_err)?;
    Ok(Json(serde_json::to_value(skills).unwrap()))
}

async fn get_skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let skill_id: SkillId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid skill id"))?;

    let skill = skill_repo::get_skill(&state.runtime.db.main, &skill_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "SKILL_NOT_FOUND", "skill not found"))?;

    Ok(Json(serde_json::to_value(skill).unwrap()))
}

async fn uninstall_skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let skill_id: SkillId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid skill id"))?;

    skill_repo::uninstall_skill(&state.runtime.db.main, &skill_id)
        .await
        .map_err(internal_err)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn enable_skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let skill_id: SkillId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid skill id"))?;

    let skill = skill_repo::enable_skill(&state.runtime.db.main, &skill_id)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(skill).unwrap()))
}

async fn disable_skill(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let skill_id: SkillId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid skill id"))?;

    let skill = skill_repo::disable_skill(&state.runtime.db.main, &skill_id)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(skill).unwrap()))
}
