//! HTTP-level tests for `POST /tools/approval/:id`.
//!
//! The route is a thin shim over `ChannelPromptGate::resolve`. These tests
//! drive it end-to-end:
//!
//!   - allow → 204 and the pending future observes `Allow`.
//!   - unknown id → 404 (request expired or never registered).
//!   - invalid decision string → 400 (before touching the gate).

use axum::http::StatusCode;
use clawx_api::build_router_for_tests;
use serde_json::json;

#[tokio::test]
async fn approval_endpoint_allow_resolves_pending_request() {
    let (router, gate) = build_router_for_tests();
    let (req_id, rx) = gate.open_request_for_test().await;

    let server = axum_test::TestServer::new(router);
    let resp = server
        .post(&format!("/tools/approval/{req_id}"))
        .json(&json!({"decision":"allow"}))
        .await;
    resp.assert_status(StatusCode::NO_CONTENT);

    let decision = rx.await.unwrap();
    assert!(matches!(decision, clawx_tools::ApprovalDecision::Allow));
}

#[tokio::test]
async fn approval_endpoint_404_on_unknown_id() {
    let (router, _gate) = build_router_for_tests();
    let server = axum_test::TestServer::new(router);
    let resp = server
        .post(&format!("/tools/approval/{}", uuid::Uuid::new_v4()))
        .json(&json!({"decision":"allow"}))
        .await;
    resp.assert_status(StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn approval_endpoint_400_on_invalid_decision() {
    let (router, gate) = build_router_for_tests();
    let (req_id, _rx) = gate.open_request_for_test().await;
    let server = axum_test::TestServer::new(router);
    let resp = server
        .post(&format!("/tools/approval/{req_id}"))
        .json(&json!({"decision":"maybe"}))
        .await;
    resp.assert_status(StatusCode::BAD_REQUEST);
}
