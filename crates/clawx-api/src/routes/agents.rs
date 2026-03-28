use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use clawx_runtime::agent_repo::{self, AgentUpdate};
use clawx_types::agent::{AgentConfig, AgentStatus};
use clawx_types::ids::{AgentId, ProviderId};
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
        .route("/", get(list_agents).post(create_agent))
        .route("/{id}", get(get_agent).put(update_agent).delete(delete_agent))
        .route("/{id}/clone", post(clone_agent))
        .route("/{id}/permission-profile", get(get_permission_profile))
}

async fn list_agents(State(state): State<Arc<AppState>>) -> ApiResult<Json<Vec<AgentConfig>>> {
    let agents = agent_repo::list_agents(&state.runtime.db.main)
        .await
        .map_err(internal_err)?;
    Ok(Json(agents))
}

#[derive(Debug, Deserialize)]
struct CreateAgentRequest {
    name: String,
    role: String,
    model_id: String,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    icon: Option<String>,
    #[serde(default)]
    capabilities: Vec<String>,
}

async fn create_agent(
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateAgentRequest>,
) -> ApiResult<(StatusCode, Json<AgentConfig>)> {
    let model_id: ProviderId = body
        .model_id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_MODEL_ID", "model_id must be a valid UUID"))?;

    let now = Utc::now();
    let agent = AgentConfig {
        id: AgentId::new(),
        name: body.name,
        role: body.role,
        system_prompt: body.system_prompt,
        model_id,
        icon: body.icon,
        status: AgentStatus::Idle,
        capabilities: body.capabilities,
        created_at: now,
        updated_at: now,
        last_active_at: None,
    };

    let created = agent_repo::create_agent(&state.runtime.db.main, &agent)
        .await
        .map_err(internal_err)?;

    Ok((StatusCode::CREATED, Json(created)))
}

async fn get_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<AgentConfig>> {
    let agent_id: AgentId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    let agent = agent_repo::get_agent(&state.runtime.db.main, &agent_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "AGENT_NOT_FOUND", "agent not found"))?;

    Ok(Json(agent))
}

#[derive(Debug, Deserialize)]
struct UpdateAgentRequest {
    name: Option<String>,
    role: Option<String>,
    system_prompt: Option<Option<String>>,
    model_id: Option<String>,
    icon: Option<Option<String>>,
    capabilities: Option<Vec<String>>,
}

async fn update_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<UpdateAgentRequest>,
) -> ApiResult<Json<AgentConfig>> {
    let agent_id: AgentId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    let updates = AgentUpdate {
        name: body.name,
        role: body.role,
        system_prompt: body.system_prompt,
        model_id: body.model_id,
        icon: body.icon,
        capabilities: body.capabilities,
    };

    let updated = agent_repo::update_agent(&state.runtime.db.main, &agent_id, &updates)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "AGENT_NOT_FOUND", "agent not found")
            }
            _ => internal_err(e),
        })?;

    Ok(Json(updated))
}

async fn delete_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let agent_id: AgentId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    agent_repo::delete_agent(&state.runtime.db.main, &agent_id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "AGENT_NOT_FOUND", "agent not found")
            }
            _ => internal_err(e),
        })?;

    Ok(StatusCode::NO_CONTENT)
}

async fn clone_agent(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<AgentConfig>)> {
    let agent_id: AgentId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "id must be a valid UUID"))?;

    let cloned = agent_repo::clone_agent(&state.runtime.db.main, &agent_id)
        .await
        .map_err(|e| match &e {
            clawx_types::ClawxError::NotFound { .. } => {
                err_response(StatusCode::NOT_FOUND, "AGENT_NOT_FOUND", "agent not found")
            }
            _ => internal_err(e),
        })?;

    Ok((StatusCode::CREATED, Json(cloned)))
}

async fn get_permission_profile(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let agent_id: AgentId = id
        .parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid agent id"))?;

    use clawx_runtime::permission_repo::SqlitePermissionRepo;

    let profile = SqlitePermissionRepo::get_profile(&state.runtime.db.main, &agent_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| {
            err_response(
                StatusCode::NOT_FOUND,
                "PERMISSION_PROFILE_NOT_FOUND",
                "permission profile not found for this agent",
            )
        })?;

    Ok(Json(serde_json::to_value(profile).unwrap()))
}
