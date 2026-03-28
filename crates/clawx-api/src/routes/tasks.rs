//! Task management API handlers (v0.2).
//!
//! CRUD for tasks, triggers, runs, and feedback.

use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use clawx_runtime::task_repo;
use clawx_types::autonomy::*;
use clawx_types::ids::*;

use crate::AppState;

type ApiResult<T> = std::result::Result<T, (StatusCode, Json<Value>)>;

fn err_response(status: StatusCode, code: &str, message: &str) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
}

fn internal_err(msg: impl std::fmt::Display) -> (StatusCode, Json<Value>) {
    err_response(StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", &msg.to_string())
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/", get(list_tasks).post(create_task))
        .route("/{id}", get(get_task).put(update_task).delete(delete_task))
        .route("/{id}/pause", post(pause_task))
        .route("/{id}/resume", post(resume_task))
        .route("/{id}/archive", post(archive_task))
        .route("/{id}/triggers", get(list_triggers).post(add_trigger))
        .route("/{id}/runs", get(list_runs))
}

// Separate router for trigger and run operations (mounted at different paths)
pub fn trigger_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/{id}", axum::routing::put(update_trigger).delete(delete_trigger))
}

pub fn run_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/{id}", get(get_run))
        .route("/{id}/feedback", post(submit_feedback))
}

// ---------------------------------------------------------------------------
// Task handlers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateTaskPayload {
    agent_id: String,
    name: String,
    goal: String,
    #[serde(default = "default_source_kind")]
    source_kind: String,
    #[serde(default = "default_max_steps")]
    default_max_steps: u32,
    #[serde(default = "default_timeout_secs")]
    default_timeout_secs: u32,
    #[serde(default)]
    notification_policy: Value,
}

fn default_source_kind() -> String { "manual".into() }
fn default_max_steps() -> u32 { 10 }
fn default_timeout_secs() -> u32 { 1800 }

async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateTaskPayload>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let agent_id: AgentId = payload.agent_id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id format"))?;

    let source_kind: TaskSourceKind = payload.source_kind.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_SOURCE_KIND", "invalid source_kind"))?;

    let now = Utc::now();
    let task = Task {
        id: TaskId::new(),
        agent_id,
        name: payload.name,
        goal: payload.goal,
        source_kind,
        lifecycle_status: TaskLifecycleStatus::Active,
        default_max_steps: payload.default_max_steps,
        default_timeout_secs: payload.default_timeout_secs,
        notification_policy: payload.notification_policy,
        suppression_state: SuppressionState::Normal,
        last_run_at: None,
        next_run_at: None,
        created_at: now,
        updated_at: now,
    };

    let task_id = task_repo::create_task(&state.runtime.db.main, &task)
        .await
        .map_err(internal_err)?;

    let created = task_repo::get_task(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| internal_err("task not found after create"))?;

    Ok((StatusCode::CREATED, Json(serde_json::to_value(created).unwrap())))
}

#[derive(Deserialize, Default)]
#[allow(dead_code)]
struct TaskQuery {
    agent_id: Option<String>,
    #[serde(default = "default_page")]
    page: u64,
    #[serde(default = "default_per_page")]
    per_page: u64,
}

fn default_page() -> u64 { 1 }
fn default_per_page() -> u64 { 20 }

async fn list_tasks(
    State(state): State<Arc<AppState>>,
    Query(query): Query<TaskQuery>,
) -> ApiResult<Json<Value>> {
    let agent_id = match &query.agent_id {
        Some(id) => Some(id.parse::<AgentId>()
            .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_AGENT_ID", "invalid agent_id"))?),
        None => None,
    };

    let tasks = task_repo::list_tasks(&state.runtime.db.main, agent_id)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(tasks).unwrap()))
}

async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let task_id: TaskId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    let task = task_repo::get_task(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "TASK_NOT_FOUND", "task not found"))?;

    Ok(Json(serde_json::to_value(task).unwrap()))
}

#[derive(Deserialize)]
struct UpdateTaskPayload {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    goal: Option<String>,
    #[serde(default)]
    notification_policy: Option<Value>,
    #[serde(default)]
    default_max_steps: Option<u32>,
    #[serde(default)]
    default_timeout_secs: Option<u32>,
}

async fn update_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTaskPayload>,
) -> ApiResult<Json<Value>> {
    let task_id: TaskId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    task_repo::update_task(
        &state.runtime.db.main,
        task_id,
        payload.name.as_deref(),
        payload.goal.as_deref(),
        payload.notification_policy.as_ref(),
        payload.default_max_steps,
        payload.default_timeout_secs,
    )
    .await
    .map_err(internal_err)?;

    let updated = task_repo::get_task(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "TASK_NOT_FOUND", "task not found"))?;

    Ok(Json(serde_json::to_value(updated).unwrap()))
}

async fn delete_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let task_id: TaskId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    task_repo::delete_task(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn pause_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let task_id: TaskId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    task_repo::update_lifecycle(&state.runtime.db.main, task_id, "paused")
        .await
        .map_err(internal_err)?;

    let task = task_repo::get_task(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "TASK_NOT_FOUND", "task not found"))?;

    Ok(Json(serde_json::to_value(task).unwrap()))
}

async fn resume_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let task_id: TaskId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    task_repo::update_lifecycle(&state.runtime.db.main, task_id, "active")
        .await
        .map_err(internal_err)?;

    let task = task_repo::get_task(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "TASK_NOT_FOUND", "task not found"))?;

    Ok(Json(serde_json::to_value(task).unwrap()))
}

async fn archive_task(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let task_id: TaskId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    task_repo::update_lifecycle(&state.runtime.db.main, task_id, "archived")
        .await
        .map_err(internal_err)?;

    let task = task_repo::get_task(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "TASK_NOT_FOUND", "task not found"))?;

    Ok(Json(serde_json::to_value(task).unwrap()))
}

// ---------------------------------------------------------------------------
// Trigger handlers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateTriggerPayload {
    trigger_kind: String,
    trigger_config: Value,
    #[serde(default)]
    next_fire_at: Option<String>,
}

async fn add_trigger(
    State(state): State<Arc<AppState>>,
    Path(task_id_str): Path<String>,
    Json(payload): Json<CreateTriggerPayload>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let task_id: TaskId = task_id_str.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    let trigger_kind: TriggerKind = payload.trigger_kind.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_TRIGGER_KIND", "invalid trigger_kind"))?;

    let next_fire_at = match &payload.next_fire_at {
        Some(s) => Some(chrono::DateTime::parse_from_rfc3339(s)
            .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_DATE", "invalid next_fire_at"))?
            .with_timezone(&Utc)),
        None => None,
    };

    let now = Utc::now();
    let trigger = Trigger {
        id: TriggerId::new(),
        task_id,
        trigger_kind,
        trigger_config: payload.trigger_config,
        status: TriggerStatus::Active,
        next_fire_at,
        last_fired_at: None,
        created_at: now,
        updated_at: now,
    };

    let trigger_id = task_repo::create_trigger(&state.runtime.db.main, &trigger)
        .await
        .map_err(internal_err)?;

    let created = task_repo::get_trigger(&state.runtime.db.main, trigger_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| internal_err("trigger not found after create"))?;

    Ok((StatusCode::CREATED, Json(serde_json::to_value(created).unwrap())))
}

async fn list_triggers(
    State(state): State<Arc<AppState>>,
    Path(task_id_str): Path<String>,
) -> ApiResult<Json<Value>> {
    let task_id: TaskId = task_id_str.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    let triggers = task_repo::list_triggers(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(triggers).unwrap()))
}

#[derive(Deserialize)]
struct UpdateTriggerPayload {
    #[serde(default)]
    trigger_config: Option<Value>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    next_fire_at: Option<String>,
}

async fn update_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<UpdateTriggerPayload>,
) -> ApiResult<Json<Value>> {
    let trigger_id: TriggerId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid trigger id"))?;

    task_repo::update_trigger(
        &state.runtime.db.main,
        trigger_id,
        payload.trigger_config.as_ref(),
        payload.status.as_deref(),
        payload.next_fire_at.as_deref(),
    )
    .await
    .map_err(internal_err)?;

    let updated = task_repo::get_trigger(&state.runtime.db.main, trigger_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "TRIGGER_NOT_FOUND", "trigger not found"))?;

    Ok(Json(serde_json::to_value(updated).unwrap()))
}

async fn delete_trigger(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let trigger_id: TriggerId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid trigger id"))?;

    task_repo::delete_trigger(&state.runtime.db.main, trigger_id)
        .await
        .map_err(internal_err)?;

    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Run handlers
// ---------------------------------------------------------------------------

async fn list_runs(
    State(state): State<Arc<AppState>>,
    Path(task_id_str): Path<String>,
) -> ApiResult<Json<Value>> {
    let task_id: TaskId = task_id_str.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid task id"))?;

    let runs = task_repo::list_runs(&state.runtime.db.main, task_id)
        .await
        .map_err(internal_err)?;

    Ok(Json(serde_json::to_value(runs).unwrap()))
}

async fn get_run(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let run_id: RunId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid run id"))?;

    let run = task_repo::get_run(&state.runtime.db.main, run_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "RUN_NOT_FOUND", "run not found"))?;

    Ok(Json(serde_json::to_value(run).unwrap()))
}

#[derive(Deserialize)]
struct FeedbackPayload {
    kind: String,
    #[serde(default)]
    reason: Option<String>,
}

async fn submit_feedback(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<FeedbackPayload>,
) -> ApiResult<Json<Value>> {
    let run_id: RunId = id.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_ID", "invalid run id"))?;

    let _feedback_kind: FeedbackKind = payload.kind.parse()
        .map_err(|_| err_response(StatusCode::BAD_REQUEST, "INVALID_FEEDBACK_KIND", "invalid feedback kind"))?;

    task_repo::record_feedback(
        &state.runtime.db.main,
        run_id,
        &payload.kind,
        payload.reason.as_deref(),
    )
    .await
    .map_err(internal_err)?;

    let run = task_repo::get_run(&state.runtime.db.main, run_id)
        .await
        .map_err(internal_err)?
        .ok_or_else(|| err_response(StatusCode::NOT_FOUND, "RUN_NOT_FOUND", "run not found"))?;

    Ok(Json(serde_json::to_value(run).unwrap()))
}
