//! Tool approval API handlers.
//!
//! Exposes `POST /tools/approval/:id` so the GUI (or any other client) can
//! answer a `Prompt`-tier tool-use approval in-session. The endpoint is a
//! thin shim over a shared `Arc<ChannelPromptGate>` — the same gate instance
//! wired into `RuleApprovalGate::with_prompt(...)` in the runtime, so
//! resolving a pending id here unblocks the in-flight tool call.
//!
//! Responses:
//!   - 204 NO_CONTENT on success.
//!   - 404 NOT_FOUND when the id is unknown (expired, already resolved,
//!     or never registered).
//!   - 400 BAD_REQUEST when the `decision` field is not `"allow"`/`"deny"`.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::post;
use axum::{Json, Router};
use clawx_tools::{ApprovalDecision, ChannelPromptGate};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ApprovalBody {
    pub decision: String,
    #[serde(default)]
    pub reason: Option<String>,
}

pub fn router(gate: Arc<ChannelPromptGate>) -> Router {
    Router::new()
        .route("/tools/approval/{id}", post(resolve))
        .with_state(gate)
}

async fn resolve(
    State(gate): State<Arc<ChannelPromptGate>>,
    Path(id): Path<Uuid>,
    Json(body): Json<ApprovalBody>,
) -> StatusCode {
    let decision = match body.decision.as_str() {
        "allow" => ApprovalDecision::Allow,
        "deny" => ApprovalDecision::Deny {
            reason: body.reason.unwrap_or_else(|| "denied by user".into()),
        },
        _ => return StatusCode::BAD_REQUEST,
    };
    if gate.resolve(id, decision).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
