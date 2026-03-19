//! System health and stats API handlers.

use std::sync::Arc;
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};
use crate::AppState;

async fn health() -> (StatusCode, Json<Value>) {
    (
        StatusCode::OK,
        Json(json!({
            "status": "ok",
            "version": "0.1.0"
        })),
    )
}

async fn stats() -> (StatusCode, Json<Value>) {
    // TODO: wire up actual stats from runtime DB
    (
        StatusCode::OK,
        Json(json!({
            "agents": 0,
            "memories": 0,
            "snapshots": 0,
            "providers": 0
        })),
    )
}

async fn not_implemented() -> (StatusCode, Json<Value>) {
    (
        StatusCode::NOT_IMPLEMENTED,
        Json(json!({"error": "not implemented"})),
    )
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/stats", get(stats))
        .route("/audit", get(not_implemented))
}
