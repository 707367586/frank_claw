//! LLM provider (model) management API handlers.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use clawx_runtime::model_repo::{self, ProviderUpdate};
use clawx_types::ids::ProviderId;
use clawx_types::llm::{LlmProviderConfig, ProviderType};
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
        .route("/", get(list_providers).post(create_provider))
        .route(
            "/{id}",
            get(get_provider).put(update_provider).delete(delete_provider),
        )
}

async fn list_providers(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Vec<LlmProviderConfig>>> {
    let providers = model_repo::list_providers(&state.runtime.db.main)
        .await
        .map_err(internal_err)?;
    Ok(Json(providers))
}

#[derive(Debug, Deserialize)]
struct CreateProviderRequest {
    name: String,
    provider_type: String,
    base_url: String,
    model_name: String,
    #[serde(default)]
    parameters: serde_json::Value,
    #[serde(default)]
    is_default: bool,
}

async fn create_provider(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateProviderRequest>,
) -> ApiResult<(StatusCode, Json<LlmProviderConfig>)> {
    let provider_type: ProviderType = body
        .provider_type
        .parse()
        .map_err(|e: String| err_response(StatusCode::BAD_REQUEST, "INVALID_PROVIDER_TYPE", &e))?;

    let now = Utc::now();
    let config = LlmProviderConfig {
        id: ProviderId::new(),
        name: body.name,
        provider_type,
        base_url: body.base_url,
        model_name: body.model_name,
        parameters: body.parameters,
        is_default: body.is_default,
        created_at: now,
        updated_at: now,
    };

    let created = model_repo::create_provider(&state.runtime.db.main, &config)
        .await
        .map_err(internal_err)?;

    Ok((StatusCode::CREATED, Json(created)))
}

async fn get_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<LlmProviderConfig>> {
    let provider_id: ProviderId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    let provider = model_repo::get_provider(&state.runtime.db.main, &provider_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "PROVIDER_NOT_FOUND", "provider not found"))?;

    Ok(Json(provider))
}

async fn update_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<ProviderUpdate>,
) -> ApiResult<Json<LlmProviderConfig>> {
    let provider_id: ProviderId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    let updated = model_repo::update_provider(&state.runtime.db.main, &provider_id, &body)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "PROVIDER_NOT_FOUND", "provider not found")
            }
            _ => internal_err(e),
        })?;

    Ok(Json(updated))
}

async fn delete_provider(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let provider_id: ProviderId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    model_repo::delete_provider(&state.runtime.db.main, &provider_id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "PROVIDER_NOT_FOUND", "provider not found")
            }
            _ => internal_err(e),
        })?;

    Ok(StatusCode::NO_CONTENT)
}
