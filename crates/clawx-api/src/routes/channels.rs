//! Channel management API handlers (v0.2).

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use clawx_runtime::channel_repo;
use clawx_types::channel::*;
use clawx_types::ids::*;
use clawx_types::traits::ChannelUpdate;

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
        .route("/", get(list_channels).post(create_channel))
        .route("/{id}", get(get_channel).put(update_channel).delete(delete_channel))
        .route("/{id}/connect", axum::routing::post(connect_channel))
        .route("/{id}/disconnect", axum::routing::post(disconnect_channel))
}

#[derive(Deserialize)]
struct CreateChannelPayload {
    channel_type: String,
    name: String,
    config: Value,
    #[serde(default)]
    agent_id: Option<String>,
}

async fn create_channel(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateChannelPayload>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let channel_type: ChannelType = payload.channel_type.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_CHANNEL_TYPE", "invalid channel type"))?;

    let agent_id = match &payload.agent_id {
        Some(id) => Some(id.parse::<AgentId>()
            .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id"))?),
        None => None,
    };

    let now = Utc::now();
    let channel = Channel {
        id: ChannelId::new(),
        channel_type,
        name: payload.name,
        config: payload.config,
        agent_id,
        status: ChannelStatus::Disconnected,
        created_at: now,
        updated_at: now,
    };

    let created = channel_repo::create_channel(&state.runtime.db.main, &channel)
        .await
        .map_err(internal_err)?;

    Ok((StatusCode::CREATED, Json(serde_json::to_value(created).unwrap())))
}

async fn list_channels(
    State(state): State<Arc<AppState>>,
) -> ApiResult<Json<Value>> {
    let channels = channel_repo::list_channels(&state.runtime.db.main)
        .await
        .map_err(internal_err)?;
    Ok(Json(serde_json::to_value(channels).unwrap()))
}

async fn get_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let channel_id: ChannelId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid channel id"))?;

    let channel = channel_repo::get_channel(&state.runtime.db.main, &channel_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "CHANNEL_NOT_FOUND", "channel not found"))?;

    Ok(Json(serde_json::to_value(channel).unwrap()))
}

#[derive(Deserialize)]
struct UpdateChannelPayload {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    config: Option<Value>,
    #[serde(default)]
    agent_id: Option<String>,
    #[serde(default)]
    status: Option<String>,
}

async fn update_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateChannelPayload>,
) -> ApiResult<Json<Value>> {
    let channel_id: ChannelId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid channel id"))?;

    let agent_id = match &payload.agent_id {
        Some(id) => Some(id.parse::<AgentId>()
            .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id"))?),
        None => None,
    };

    let status = match &payload.status {
        Some(s) => Some(s.parse::<ChannelStatus>()
            .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_STATUS", "invalid status"))?),
        None => None,
    };

    let update = ChannelUpdate {
        name: payload.name,
        config: payload.config,
        agent_id,
        status,
    };

    let updated = channel_repo::update_channel(&state.runtime.db.main, &channel_id, &update)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(updated).unwrap()))
}

async fn delete_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let channel_id: ChannelId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid channel id"))?;

    channel_repo::delete_channel(&state.runtime.db.main, &channel_id)
        .await
        .map_err(internal_err)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn connect_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let channel_id: ChannelId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid channel id"))?;

    // Update status to connected
    let update = ChannelUpdate {
        status: Some(ChannelStatus::Connected),
        ..Default::default()
    };

    let updated = channel_repo::update_channel(&state.runtime.db.main, &channel_id, &update)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(updated).unwrap()))
}

async fn disconnect_channel(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let channel_id: ChannelId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid channel id"))?;

    // Update status to disconnected
    let update = ChannelUpdate {
        status: Some(ChannelStatus::Disconnected),
        ..Default::default()
    };

    let updated = channel_repo::update_channel(&state.runtime.db.main, &channel_id, &update)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(updated).unwrap()))
}
