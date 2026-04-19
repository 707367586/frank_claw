//! RESTful API server for ClawX.
//!
//! Exposes agent runtime operations over HTTP/UDS using Axum.
//! All routes return 501 Not Implemented in the skeleton phase.

mod routes;
mod middleware;

use std::sync::Arc;

use axum::http::{HeaderValue, Method};
use axum::Router;
use clawx_runtime::Runtime;
use clawx_tools::ChannelPromptGate;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing::info;

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    pub runtime: Runtime,
    pub control_token: String,
}

/// Build the complete API router with all route groups.
///
/// A permissive CORS layer is installed so the Vite dev server and the Tauri
/// webview (which load on `http://localhost:1420` / `tauri://localhost`) can
/// reach the API running on `http://127.0.0.1:9090`. Browser same-origin policy
/// otherwise blocks every preflight. The auth middleware still gates every
/// non-OPTIONS request by bearer token.
pub fn build_router(state: AppState) -> Router {
    let shared = Arc::new(state);

    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(|origin: &HeaderValue, _| {
            origin
                .to_str()
                .ok()
                .map(|s| {
                    s.starts_with("http://localhost")
                        || s.starts_with("http://127.0.0.1")
                        || s.starts_with("tauri://")
                })
                .unwrap_or(false)
        }))
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers(tower_http::cors::Any)
        .max_age(std::time::Duration::from_secs(86400));

    // The prompt gate is owned by the router and shared with whatever path
    // wires it into the runtime's approval port. Production wiring (i.e.
    // `RuleApprovalGate::with_prompt(gate.clone())`) is set up by the
    // service binary; see ADR notes in T11.
    let gate = ChannelPromptGate::new();

    Router::new()
        .nest("/agents", routes::agents::router())
        .nest("/conversations", routes::conversations::router())
        .nest("/memories", routes::memories::router())
        .nest("/knowledge", routes::knowledge::router())
        .nest("/vault", routes::vault::router())
        .nest("/models", routes::models::router())
        .nest("/system", routes::system::router())
        // v0.2 routes
        .nest("/tasks", routes::tasks::router())
        .nest("/task-triggers", routes::tasks::trigger_router())
        .nest("/task-runs", routes::tasks::run_router())
        .nest("/channels", routes::channels::router())
        .nest("/skills", routes::skills::router())
        .layer(axum::middleware::from_fn_with_state(
            shared.clone(),
            middleware::auth::require_token,
        ))
        .layer(cors)
        .with_state(shared)
        // Tools approval sits outside the AppState-bound middleware chain:
        // the gate is its own state, and the endpoint is a short POST with
        // no token dependency (the SwiftUI app reaches it via the loopback
        // UDS/TCP socket that the service binds).
        .merge(routes::tools::router(gate))
}

/// Test harness: build an approval-only router wired to a fresh
/// `ChannelPromptGate`. The caller can register pending requests on `gate`
/// via `open_request_for_test`, fire HTTP requests at the returned router,
/// and assert on the pending future's resolution.
pub fn build_router_for_tests() -> (Router, Arc<ChannelPromptGate>) {
    let gate = ChannelPromptGate::new();
    let router = routes::tools::router(gate.clone());
    (router, gate)
}

/// Start the API server on a Unix Domain Socket.
pub async fn serve_uds(router: Router, socket_path: &str) -> std::io::Result<()> {
    // Remove stale socket
    let _ = tokio::fs::remove_file(socket_path).await;
    if let Some(parent) = std::path::Path::new(socket_path).parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let listener = tokio::net::UnixListener::bind(socket_path)?;
    info!(path = socket_path, "API server listening on UDS");
    axum::serve(listener, router).await
}

/// Start the API server on a TCP port (dev mode).
pub async fn serve_tcp(router: Router, port: u16) -> std::io::Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!(%addr, "API server listening on TCP (dev mode)");
    axum::serve(listener, router).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    async fn make_state() -> AppState {
        make_state_with_llm(Arc::new(clawx_llm::StubLlmProvider)).await
    }

    /// Build an `AppState` backed by an in-memory DB and stub services, but
    /// with a caller-supplied LLM provider. Used by streaming tests that
    /// need to control the exact chunk sequence / error behavior.
    async fn make_state_with_llm(
        llm: Arc<dyn clawx_types::traits::LlmProvider>,
    ) -> AppState {
        AppState {
            runtime: Runtime::new(
                clawx_runtime::db::Database::in_memory().await.unwrap(),
                llm,
                Arc::new(clawx_memory::StubMemoryService),
                Arc::new(clawx_memory::StubWorkingMemoryManager),
                Arc::new(clawx_memory::StubMemoryExtractor),
                Arc::new(clawx_security::PermissiveSecurityGuard),
                Arc::new(clawx_vault::StubVaultService),
                Arc::new(clawx_kb::StubKnowledgeService),
                Arc::new(clawx_config::ConfigLoader::with_defaults()),
            ),
            control_token: "test-token-123".to_string(),
        }
    }

    async fn request(
        router: &Router,
        method: &str,
        path: &str,
        token: &str,
    ) -> (u16, String) {
        let req = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("Authorization", format!("Bearer {}", token))
            .body(Body::empty())
            .unwrap();

        let resp = router.clone().oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&body).to_string();
        (status, text)
    }

    #[tokio::test]
    async fn system_health_returns_200() {
        let router = build_router(make_state().await);
        let (status, body) = request(&router, "GET", "/system/health", "test-token-123").await;
        assert_eq!(status, 200);
        let json: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["version"], "0.1.0");
    }

    #[tokio::test]
    async fn agents_list_returns_empty_array() {
        let router = build_router(make_state().await);
        let (status, body) = request(&router, "GET", "/agents", "test-token-123").await;
        assert_eq!(status, 200);
        let agents: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(agents.is_empty());
    }

    #[tokio::test]
    async fn unauthorized_request_returns_401() {
        let router = build_router(make_state().await);
        let (status, _) = request(&router, "GET", "/system/health", "wrong-token").await;
        assert_eq!(status, 401);
    }

    #[tokio::test]
    async fn all_route_groups_return_200() {
        let router = build_router(make_state().await);
        let token = "test-token-123";

        // All implemented routes should return 200
        let routes_200 = [
            "/agents",
            "/models",
            "/vault",
            "/memories/stats",
            "/knowledge/sources",
        ];
        for path in routes_200 {
            let (status, _) = request(&router, "GET", path, token).await;
            assert_eq!(status, 200, "route {} should return 200", path);
        }
        // System health returns 200
        let (status, _) = request(&router, "GET", "/system/health", token).await;
        assert_eq!(status, 200, "/system/health should return 200");
    }

    // -----------------------------------------------------------------------
    // Models CRUD tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn models_list_returns_empty() {
        let router = build_router(make_state().await);
        let (status, body) = request(&router, "GET", "/models", "test-token-123").await;
        assert_eq!(status, 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn models_crud_full_flow() {
        let router = build_router(make_state().await);
        let token = "test-token-123";

        // Create
        let (status, body) = json_request(
            &router, "POST", "/models", token,
            serde_json::json!({
                "name": "GPT-4",
                "provider_type": "openai",
                "base_url": "https://api.openai.com",
                "model_name": "gpt-4",
                "parameters": {"temperature": 0.7},
                "is_default": true
            }),
        ).await;
        assert_eq!(status, 201, "create model should return 201, body: {}", body);
        let model: serde_json::Value = serde_json::from_str(&body).unwrap();
        let model_id = model["id"].as_str().unwrap().to_string();
        assert_eq!(model["name"], "GPT-4");
        assert_eq!(model["provider_type"], "openai");
        assert!(model["is_default"].as_bool().unwrap());

        // Get
        let (status, body) = request(&router, "GET", &format!("/models/{}", model_id), token).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["name"], "GPT-4");

        // List
        let (status, body) = request(&router, "GET", "/models", token).await;
        assert_eq!(status, 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(list.len(), 1);

        // Update
        let (status, body) = json_request(
            &router, "PUT", &format!("/models/{}", model_id), token,
            serde_json::json!({"name": "GPT-4 Turbo"}),
        ).await;
        assert_eq!(status, 200);
        let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(updated["name"], "GPT-4 Turbo");
        assert_eq!(updated["model_name"], "gpt-4"); // unchanged

        // Delete
        let (status, _) = request(&router, "DELETE", &format!("/models/{}", model_id), token).await;
        assert_eq!(status, 204);

        // Get deleted → 404
        let (status, _) = request(&router, "GET", &format!("/models/{}", model_id), token).await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn models_get_not_found() {
        let router = build_router(make_state().await);
        let fake_id = uuid::Uuid::new_v4();
        let (status, body) = request(
            &router, "GET", &format!("/models/{}", fake_id), "test-token-123",
        ).await;
        assert_eq!(status, 404);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "PROVIDER_NOT_FOUND");
    }

    #[tokio::test]
    async fn models_create_invalid_provider_type() {
        let router = build_router(make_state().await);
        let (status, body) = json_request(
            &router, "POST", "/models", "test-token-123",
            serde_json::json!({
                "name": "Bad",
                "provider_type": "invalid-type",
                "base_url": "https://example.com",
                "model_name": "bad",
            }),
        ).await;
        assert_eq!(status, 400);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "INVALID_PROVIDER_TYPE");
    }

    // -----------------------------------------------------------------------
    // Conversations CRUD tests
    // -----------------------------------------------------------------------

    /// Helper: create an agent and return its ID.
    async fn create_test_agent(router: &Router, token: &str) -> String {
        let model_id = uuid::Uuid::new_v4().to_string();
        let (_, body) = json_request(
            router, "POST", "/agents", token,
            serde_json::json!({
                "name": "Test Bot",
                "role": "assistant",
                "model_id": model_id,
            }),
        ).await;
        let agent: serde_json::Value = serde_json::from_str(&body).unwrap();
        agent["id"].as_str().unwrap().to_string()
    }

    #[tokio::test]
    async fn conversations_crud_full_flow() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let agent_id = create_test_agent(&router, token).await;

        // Create conversation
        let (status, body) = json_request(
            &router, "POST", "/conversations", token,
            serde_json::json!({
                "agent_id": agent_id,
                "title": "Test Chat"
            }),
        ).await;
        assert_eq!(status, 201, "create conv should return 201, body: {}", body);
        let conv: serde_json::Value = serde_json::from_str(&body).unwrap();
        let conv_id = conv["id"].as_str().unwrap().to_string();
        assert_eq!(conv["title"], "Test Chat");
        assert_eq!(conv["agent_id"], agent_id);

        // Get conversation
        let (status, body) = request(&router, "GET", &format!("/conversations/{}", conv_id), token).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["title"], "Test Chat");

        // List conversations
        let (status, body) = request(
            &router, "GET",
            &format!("/conversations?agent_id={}", agent_id), token,
        ).await;
        assert_eq!(status, 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(list.len(), 1);

        // Add message
        let (status, body) = json_request(
            &router, "POST", &format!("/conversations/{}/messages", conv_id), token,
            serde_json::json!({"role": "user", "content": "Hello!"}),
        ).await;
        assert_eq!(status, 201);
        let msg: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(msg["role"], "user");
        assert_eq!(msg["content"], "Hello!");

        // List messages
        let (status, body) = request(
            &router, "GET", &format!("/conversations/{}/messages", conv_id), token,
        ).await;
        assert_eq!(status, 200);
        let msgs: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["content"], "Hello!");

        // Delete conversation
        let (status, _) = request(&router, "DELETE", &format!("/conversations/{}", conv_id), token).await;
        assert_eq!(status, 204);

        // Get deleted → 404
        let (status, _) = request(&router, "GET", &format!("/conversations/{}", conv_id), token).await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn list_conversations_without_agent_id_returns_200() {
        let router = build_router(make_state().await);
        let (status, _body) =
            request(&router, "GET", "/conversations", "test-token-123").await;
        assert_eq!(status, 200, "expected 200, got {}", status);

        let list: Vec<serde_json::Value> =
            serde_json::from_str(&_body).expect("body must be JSON array");
        assert!(list.is_empty(), "expected empty list with no conversations seeded, got {:?}", list);
    }

    #[tokio::test]
    async fn conversations_get_not_found() {
        let router = build_router(make_state().await);
        let (status, body) = request(
            &router, "GET", &format!("/conversations/{}", uuid::Uuid::new_v4()), "test-token-123",
        ).await;
        assert_eq!(status, 404);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "CONVERSATION_NOT_FOUND");
    }

    #[tokio::test]
    async fn conversations_add_message_to_nonexistent() {
        let router = build_router(make_state().await);
        let (status, body) = json_request(
            &router, "POST",
            &format!("/conversations/{}/messages", uuid::Uuid::new_v4()),
            "test-token-123",
            serde_json::json!({"role": "user", "content": "Hello!"}),
        ).await;
        assert_eq!(status, 404);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "CONVERSATION_NOT_FOUND");
    }

    // -----------------------------------------------------------------------
    // Memories API tests (uses StubMemoryService)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn memories_stats_returns_zeros() {
        let router = build_router(make_state().await);
        let (status, body) = request(&router, "GET", "/memories/stats", "test-token-123").await;
        assert_eq!(status, 200);
        let stats: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(stats["total_count"], 0);
    }

    #[tokio::test]
    async fn memories_search_returns_empty() {
        let router = build_router(make_state().await);
        let (status, body) = json_request(
            &router, "POST", "/memories/search", "test-token-123",
            serde_json::json!({"query_text": "test", "top_k": 5}),
        ).await;
        assert_eq!(status, 200);
        let results: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(results.is_empty());
    }

    // -----------------------------------------------------------------------
    // Vault API tests (uses StubVaultService)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn vault_list_returns_empty() {
        let router = build_router(make_state().await);
        let (status, body) = request(&router, "GET", "/vault", "test-token-123").await;
        assert_eq!(status, 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(list.is_empty());
    }

    // -----------------------------------------------------------------------
    // Knowledge API tests (uses StubKnowledgeService)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn knowledge_sources_returns_empty() {
        let router = build_router(make_state().await);
        let (status, body) = request(&router, "GET", "/knowledge/sources", "test-token-123").await;
        assert_eq!(status, 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn knowledge_search_returns_empty() {
        let router = build_router(make_state().await);
        let (status, body) = json_request(
            &router, "POST", "/knowledge/search", "test-token-123",
            serde_json::json!({"query_text": "rust async", "top_n": 5}),
        ).await;
        assert_eq!(status, 200);
        let results: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(results.is_empty());
    }

    async fn json_request(
        router: &Router,
        method: &str,
        path: &str,
        token: &str,
        body: serde_json::Value,
    ) -> (u16, String) {
        let req = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();

        let resp = router.clone().oneshot(req).await.unwrap();
        let status = resp.status().as_u16();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).to_string();
        (status, text)
    }

    #[tokio::test]
    async fn agent_crud_full_flow() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let model_id = uuid::Uuid::new_v4().to_string();

        // Create
        let (status, body) = json_request(
            &router, "POST", "/agents", token,
            serde_json::json!({
                "name": "Test Bot",
                "role": "assistant",
                "model_id": model_id,
                "system_prompt": "Be helpful",
                "capabilities": ["web_search"]
            }),
        ).await;
        assert_eq!(status, 201, "create should return 201, body: {}", body);
        let agent: serde_json::Value = serde_json::from_str(&body).unwrap();
        let agent_id = agent["id"].as_str().unwrap().to_string();
        assert_eq!(agent["name"], "Test Bot");
        assert_eq!(agent["status"], "idle");

        // Get
        let (status, body) = request(&router, "GET", &format!("/agents/{}", agent_id), token).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["name"], "Test Bot");

        // List
        let (status, body) = request(&router, "GET", "/agents", token).await;
        assert_eq!(status, 200);
        let list: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(list.len(), 1);

        // Update
        let (status, body) = json_request(
            &router, "PUT", &format!("/agents/{}", agent_id), token,
            serde_json::json!({ "name": "Renamed Bot" }),
        ).await;
        assert_eq!(status, 200);
        let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(updated["name"], "Renamed Bot");
        assert_eq!(updated["role"], "assistant"); // unchanged

        // Clone
        let (status, body) = json_request(
            &router, "POST", &format!("/agents/{}/clone", agent_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 201);
        let cloned: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(cloned["name"], "Renamed Bot (Copy)");
        assert_ne!(cloned["id"], agent_id);

        // Delete
        let (status, _) = request(&router, "DELETE", &format!("/agents/{}", agent_id), token).await;
        assert_eq!(status, 204);

        // Get deleted → 404
        let (status, _) = request(&router, "GET", &format!("/agents/{}", agent_id), token).await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn agent_get_not_found() {
        let router = build_router(make_state().await);
        let fake_id = uuid::Uuid::new_v4();
        let (status, body) = request(
            &router, "GET", &format!("/agents/{}", fake_id), "test-token-123",
        ).await;
        assert_eq!(status, 404);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "AGENT_NOT_FOUND");
    }

    #[tokio::test]
    async fn agent_create_invalid_model_id() {
        let router = build_router(make_state().await);
        let (status, body) = json_request(
            &router, "POST", "/agents", "test-token-123",
            serde_json::json!({
                "name": "Bad",
                "role": "assistant",
                "model_id": "not-a-uuid",
            }),
        ).await;
        assert_eq!(status, 400);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "INVALID_MODEL_ID");
    }

    // -----------------------------------------------------------------------
    // Skills API tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn skills_list_returns_empty() {
        let router = build_router(make_state().await);
        let (status, body) = request(&router, "GET", "/skills", "test-token-123").await;
        assert_eq!(status, 200);
        let skills: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(skills.is_empty());
    }

    #[tokio::test]
    async fn skills_install_and_list() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let wasm_hex = hex::encode(b"fake-wasm-bytes");

        // Install
        let (status, body) = json_request(
            &router, "POST", "/skills", token,
            serde_json::json!({
                "manifest": {
                    "name": "greeting",
                    "version": "1.0.0",
                    "entrypoint": "main.wasm",
                    "capabilities": {}
                },
                "wasm_bytes_hex": wasm_hex
            }),
        ).await;
        assert_eq!(status, 201, "install should return 201, body: {}", body);
        let skill: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(skill["name"], "greeting");
        assert_eq!(skill["version"], "1.0.0");
        assert_eq!(skill["status"], "enabled");

        // List
        let (status, body) = request(&router, "GET", "/skills", token).await;
        assert_eq!(status, 200);
        let skills: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0]["name"], "greeting");
    }

    #[tokio::test]
    async fn skills_get_by_id() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let wasm_hex = hex::encode(b"wasm");

        // Install first
        let (_, body) = json_request(
            &router, "POST", "/skills", token,
            serde_json::json!({
                "manifest": {
                    "name": "fetcher",
                    "version": "2.0.0",
                    "entrypoint": "run.wasm",
                    "capabilities": { "net_http": ["api.example.com"] }
                },
                "wasm_bytes_hex": wasm_hex
            }),
        ).await;
        let skill: serde_json::Value = serde_json::from_str(&body).unwrap();
        let skill_id = skill["id"].as_str().unwrap();

        // Get by ID
        let (status, body) = request(&router, "GET", &format!("/skills/{}", skill_id), token).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["name"], "fetcher");
        assert_eq!(fetched["version"], "2.0.0");
    }

    #[tokio::test]
    async fn skills_get_not_found() {
        let router = build_router(make_state().await);
        let fake_id = uuid::Uuid::new_v4();
        let (status, body) = request(
            &router, "GET", &format!("/skills/{}", fake_id), "test-token-123",
        ).await;
        assert_eq!(status, 404);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "SKILL_NOT_FOUND");
    }

    #[tokio::test]
    async fn skills_uninstall() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let wasm_hex = hex::encode(b"wasm");

        // Install
        let (_, body) = json_request(
            &router, "POST", "/skills", token,
            serde_json::json!({
                "manifest": {
                    "name": "to-remove",
                    "version": "1.0.0",
                    "entrypoint": "main.wasm",
                    "capabilities": {}
                },
                "wasm_bytes_hex": wasm_hex
            }),
        ).await;
        let skill: serde_json::Value = serde_json::from_str(&body).unwrap();
        let skill_id = skill["id"].as_str().unwrap();

        // Delete
        let (status, _) = request(&router, "DELETE", &format!("/skills/{}", skill_id), token).await;
        assert_eq!(status, 204);

        // Get deleted -> 404
        let (status, _) = request(&router, "GET", &format!("/skills/{}", skill_id), token).await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn skills_enable_disable() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let wasm_hex = hex::encode(b"wasm");

        // Install
        let (_, body) = json_request(
            &router, "POST", "/skills", token,
            serde_json::json!({
                "manifest": {
                    "name": "toggle-skill",
                    "version": "1.0.0",
                    "entrypoint": "main.wasm",
                    "capabilities": {}
                },
                "wasm_bytes_hex": wasm_hex
            }),
        ).await;
        let skill: serde_json::Value = serde_json::from_str(&body).unwrap();
        let skill_id = skill["id"].as_str().unwrap();
        assert_eq!(skill["status"], "enabled");

        // Disable
        let (status, body) = json_request(
            &router, "POST", &format!("/skills/{}/disable", skill_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 200);
        let disabled: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(disabled["status"], "disabled");

        // Enable
        let (status, body) = json_request(
            &router, "POST", &format!("/skills/{}/enable", skill_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 200);
        let enabled: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(enabled["status"], "enabled");
    }

    #[tokio::test]
    async fn skills_install_invalid_hex() {
        let router = build_router(make_state().await);
        let (status, body) = json_request(
            &router, "POST", "/skills", "test-token-123",
            serde_json::json!({
                "manifest": {
                    "name": "bad",
                    "version": "1.0.0",
                    "entrypoint": "main.wasm",
                    "capabilities": {}
                },
                "wasm_bytes_hex": "not-valid-hex!!!"
            }),
        ).await;
        assert_eq!(status, 400);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "INVALID_WASM");
    }

    #[tokio::test]
    async fn skills_install_with_signature() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let wasm_hex = hex::encode(b"signed-wasm");

        let (status, body) = json_request(
            &router, "POST", "/skills", token,
            serde_json::json!({
                "manifest": {
                    "name": "signed-skill",
                    "version": "1.0.0",
                    "entrypoint": "main.wasm",
                    "capabilities": {}
                },
                "wasm_bytes_hex": wasm_hex,
                "signature": "abcdef1234567890"
            }),
        ).await;
        assert_eq!(status, 201, "install with sig should return 201, body: {}", body);
        let skill: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(skill["signature"], "abcdef1234567890");
    }

    // -----------------------------------------------------------------------
    // Task API integration tests (Phase 11)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn task_crud_full_flow() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let agent_id = create_test_agent(&router, token).await;

        // Create task
        let (status, body) = json_request(
            &router, "POST", "/tasks", token,
            serde_json::json!({
                "agent_id": agent_id,
                "name": "Daily Report",
                "goal": "Generate daily summary",
            }),
        ).await;
        assert_eq!(status, 201, "create task failed: {}", body);
        let task: serde_json::Value = serde_json::from_str(&body).unwrap();
        let task_id = task["id"].as_str().unwrap();

        // Get task
        let (status, body) = request(&router, "GET", &format!("/tasks/{}", task_id), token).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["name"], "Daily Report");
        assert_eq!(fetched["goal"], "Generate daily summary");
        assert_eq!(fetched["lifecycle_status"], "active");

        // Update task
        let (status, body) = json_request(
            &router, "PUT", &format!("/tasks/{}", task_id), token,
            serde_json::json!({"name": "Weekly Report"}),
        ).await;
        assert_eq!(status, 200);
        let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(updated["name"], "Weekly Report");
        assert_eq!(updated["goal"], "Generate daily summary"); // unchanged

        // List tasks
        let (status, body) = request(&router, "GET", "/tasks", token).await;
        assert_eq!(status, 200);
        let tasks: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(!tasks.is_empty());

        // Pause
        let (status, body) = json_request(
            &router, "POST", &format!("/tasks/{}/pause", task_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 200);
        let paused: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(paused["lifecycle_status"], "paused");

        // Resume
        let (status, body) = json_request(
            &router, "POST", &format!("/tasks/{}/resume", task_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 200);
        let resumed: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(resumed["lifecycle_status"], "active");

        // Archive
        let (status, body) = json_request(
            &router, "POST", &format!("/tasks/{}/archive", task_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 200);
        let archived: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(archived["lifecycle_status"], "archived");

        // Delete
        let (status, _) = request(&router, "DELETE", &format!("/tasks/{}", task_id), token).await;
        assert_eq!(status, 204);

        // Get deleted -> 404
        let (status, _) = request(&router, "GET", &format!("/tasks/{}", task_id), token).await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn task_get_not_found() {
        let router = build_router(make_state().await);
        let fake_id = uuid::Uuid::new_v4();
        let (status, body) = request(
            &router, "GET", &format!("/tasks/{}", fake_id), "test-token-123",
        ).await;
        assert_eq!(status, 404);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "TASK_NOT_FOUND");
    }

    #[tokio::test]
    async fn task_triggers_crud() {
        let state = make_state().await;
        let router = build_router(state.clone());
        let token = "test-token-123";
        let agent_id = create_test_agent(&router, token).await;

        // Create task first
        let (status, body) = json_request(
            &router, "POST", "/tasks", token,
            serde_json::json!({
                "agent_id": agent_id,
                "name": "Trigger Test Task",
                "goal": "Test triggers",
            }),
        ).await;
        assert_eq!(status, 201);
        let task: serde_json::Value = serde_json::from_str(&body).unwrap();
        let task_id = task["id"].as_str().unwrap();

        // Add trigger
        let (status, body) = json_request(
            &router, "POST", &format!("/tasks/{}/triggers", task_id), token,
            serde_json::json!({
                "trigger_kind": "time",
                "trigger_config": {"cron": "0 9 * * *"},
            }),
        ).await;
        assert_eq!(status, 201, "add trigger failed: {}", body);
        let trigger: serde_json::Value = serde_json::from_str(&body).unwrap();
        let trigger_id = trigger["id"].as_str().unwrap();
        assert_eq!(trigger["trigger_kind"], "time");
        assert_eq!(trigger["status"], "active");

        // List triggers
        let (status, body) = request(
            &router, "GET", &format!("/tasks/{}/triggers", task_id), token,
        ).await;
        assert_eq!(status, 200);
        let triggers: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(triggers.len(), 1);

        // Update trigger (via trigger_router at /task-triggers/:id)
        let (status, body) = json_request(
            &router, "PUT", &format!("/task-triggers/{}", trigger_id), token,
            serde_json::json!({"status": "paused"}),
        ).await;
        assert_eq!(status, 200);
        let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(updated["status"], "paused");

        // Delete trigger
        let (status, _) = request(
            &router, "DELETE", &format!("/task-triggers/{}", trigger_id), token,
        ).await;
        assert_eq!(status, 204);

        // List triggers after delete -> empty
        let (status, body) = request(
            &router, "GET", &format!("/tasks/{}/triggers", task_id), token,
        ).await;
        assert_eq!(status, 200);
        let triggers: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert!(triggers.is_empty());
    }

    #[tokio::test]
    async fn task_runs_and_feedback() {
        let state = make_state().await;
        let router = build_router(state.clone());
        let token = "test-token-123";
        let agent_id = create_test_agent(&router, token).await;

        // Create task
        let (_, body) = json_request(
            &router, "POST", "/tasks", token,
            serde_json::json!({
                "agent_id": agent_id,
                "name": "Run Test Task",
                "goal": "Test runs",
            }),
        ).await;
        let task: serde_json::Value = serde_json::from_str(&body).unwrap();
        let task_id_str = task["id"].as_str().unwrap();
        let task_id: clawx_types::ids::TaskId = task_id_str.parse().unwrap();

        // Create a run directly via the DB (no API endpoint for creating runs)
        let now = chrono::Utc::now();
        let run = clawx_types::autonomy::Run {
            id: clawx_types::autonomy::RunId::new(),
            task_id,
            trigger_id: None,
            idempotency_key: "test-key-1".to_string(),
            run_status: clawx_types::autonomy::RunStatus::Queued,
            attempt: 1,
            lease_owner: None,
            lease_expires_at: None,
            checkpoint: serde_json::json!({}),
            tokens_used: 0,
            steps_count: 0,
            result_summary: None,
            failure_reason: None,
            feedback_kind: None,
            feedback_reason: None,
            notification_status: clawx_types::autonomy::NotificationStatus::Pending,
            triggered_at: now,
            started_at: None,
            finished_at: None,
            created_at: now,
        };
        let run_id = clawx_runtime::task_repo::create_run(&state.runtime.db.main, &run)
            .await
            .unwrap();

        // List runs for task
        let (status, body) = request(
            &router, "GET", &format!("/tasks/{}/runs", task_id_str), token,
        ).await;
        assert_eq!(status, 200);
        let runs: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0]["idempotency_key"], "test-key-1");

        // Get run by ID
        let (status, body) = request(
            &router, "GET", &format!("/task-runs/{}", run_id), token,
        ).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["run_status"], "queued");

        // Submit feedback
        let (status, body) = json_request(
            &router, "POST", &format!("/task-runs/{}/feedback", run_id), token,
            serde_json::json!({"kind": "accepted", "reason": "Looks good"}),
        ).await;
        assert_eq!(status, 200);
        let fb_run: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fb_run["feedback_kind"], "accepted");
        assert_eq!(fb_run["feedback_reason"], "Looks good");
    }

    // -----------------------------------------------------------------------
    // Channel API integration tests (Phase 11)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn channel_crud_full_flow() {
        let router = build_router(make_state().await);
        let token = "test-token-123";
        let agent_id = create_test_agent(&router, token).await;

        // Create channel
        let (status, body) = json_request(
            &router, "POST", "/channels", token,
            serde_json::json!({
                "channel_type": "telegram",
                "name": "My Telegram Bot",
                "config": {"bot_token": "123:ABC"},
            }),
        ).await;
        assert_eq!(status, 201, "create channel failed: {}", body);
        let channel: serde_json::Value = serde_json::from_str(&body).unwrap();
        let channel_id = channel["id"].as_str().unwrap();
        assert_eq!(channel["name"], "My Telegram Bot");
        assert_eq!(channel["channel_type"], "telegram");
        assert_eq!(channel["status"], "disconnected");

        // Get channel
        let (status, body) = request(&router, "GET", &format!("/channels/{}", channel_id), token).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["name"], "My Telegram Bot");

        // List channels
        let (status, body) = request(&router, "GET", "/channels", token).await;
        assert_eq!(status, 200);
        let channels: Vec<serde_json::Value> = serde_json::from_str(&body).unwrap();
        assert_eq!(channels.len(), 1);

        // Update channel (bind to agent)
        let (status, body) = json_request(
            &router, "PUT", &format!("/channels/{}", channel_id), token,
            serde_json::json!({
                "name": "Renamed Bot",
                "agent_id": agent_id,
            }),
        ).await;
        assert_eq!(status, 200);
        let updated: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(updated["name"], "Renamed Bot");
        assert_eq!(updated["agent_id"], agent_id);

        // Connect (no body needed)
        let (status, body) = request(&router, "POST", &format!("/channels/{}/connect", channel_id), token).await;
        assert_eq!(status, 200);
        let connected: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(connected["status"], "connected");

        // Disconnect (no body needed)
        let (status, body) = request(&router, "POST", &format!("/channels/{}/disconnect", channel_id), token).await;
        assert_eq!(status, 200);
        let disconnected: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(disconnected["status"], "disconnected");

        // Delete
        let (status, _) = request(&router, "DELETE", &format!("/channels/{}", channel_id), token).await;
        assert_eq!(status, 204);

        // Get deleted -> 404
        let (status, _) = request(&router, "GET", &format!("/channels/{}", channel_id), token).await;
        assert_eq!(status, 404);
    }

    #[tokio::test]
    async fn channel_get_not_found() {
        let router = build_router(make_state().await);
        let fake_id = uuid::Uuid::new_v4();
        let (status, body) = request(
            &router, "GET", &format!("/channels/{}", fake_id), "test-token-123",
        ).await;
        assert_eq!(status, 404);
        let err: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(err["error"]["code"], "CHANNEL_NOT_FOUND");
    }

    // -----------------------------------------------------------------------
    // Skills enable/disable integration test (Phase 11)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn skills_enable_disable_flow() {
        let state = make_state().await;
        let router = build_router(state.clone());
        let token = "test-token-123";

        // Install a skill via DB directly
        let manifest = clawx_types::skill::SkillManifest {
            name: "test-skill".to_string(),
            version: "1.0.0".to_string(),
            description: Some("A test skill".to_string()),
            author: Some("tester".to_string()),
            entrypoint: "main.wasm".to_string(),
            capabilities: clawx_types::skill::CapabilityDeclaration::default(),
        };
        let skill = clawx_runtime::skill_repo::install_skill(
            &state.runtime.db.main,
            &manifest,
            b"fake-wasm",
            None,
        ).await.unwrap();
        let skill_id = skill.id.to_string();

        // Get skill via API
        let (status, body) = request(&router, "GET", &format!("/skills/{}", skill_id), token).await;
        assert_eq!(status, 200);
        let fetched: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(fetched["name"], "test-skill");
        assert_eq!(fetched["status"], "enabled");

        // Disable
        let (status, body) = json_request(
            &router, "POST", &format!("/skills/{}/disable", skill_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 200);
        let disabled: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(disabled["status"], "disabled");

        // Enable
        let (status, body) = json_request(
            &router, "POST", &format!("/skills/{}/enable", skill_id), token,
            serde_json::json!({}),
        ).await;
        assert_eq!(status, 200);
        let enabled: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(enabled["status"], "enabled");

        // Uninstall
        let (status, _) = request(&router, "DELETE", &format!("/skills/{}", skill_id), token).await;
        assert_eq!(status, 204);

        // Get uninstalled -> 404
        let (status, _) = request(&router, "GET", &format!("/skills/{}", skill_id), token).await;
        assert_eq!(status, 404);
    }

    // -----------------------------------------------------------------------
    // Permission profile API test (Phase 11)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn sse_persists_accumulated_content_not_placeholder() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let state = make_state().await;
        let app = build_router(state.clone());

        // 1. Build agent + conversation (StubLlmProvider emits a non-empty delta)
        let agent = clawx_types::agent::AgentConfig {
            id: clawx_types::ids::AgentId::new(),
            name: "t".into(),
            role: "t".into(),
            system_prompt: None,
            model_id: clawx_types::ids::ProviderId::new(),
            icon: None,
            status: clawx_types::agent::AgentStatus::Idle,
            capabilities: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_active_at: None,
        };
        let agent_id = clawx_runtime::agent_repo::create_agent(
            &state.runtime.db.main,
            &agent,
        )
        .await
        .unwrap()
        .id;

        let conv_id = clawx_runtime::conversation_repo::create_conversation(
            &state.runtime.db.main,
            &agent_id.to_string(),
            None,
        )
        .await
        .unwrap();

        // 2. Send a user message with stream=true and consume the SSE stream
        let body = serde_json::json!({
            "role": "user",
            "content": "ping",
            "stream": true,
        });
        let req = Request::builder()
            .method("POST")
            .uri(format!("/conversations/{}/messages", conv_id))
            .header("Authorization", "Bearer test-token-123")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = resp.into_body().collect().await.unwrap();

        // 3. The stored assistant message must contain accumulated deltas, not the placeholder
        let messages = clawx_runtime::conversation_repo::list_messages(
            &state.runtime.db.main,
            &conv_id,
        )
        .await
        .unwrap();
        let assistant = messages
            .iter()
            .find(|m| m["role"] == "assistant")
            .expect("assistant message must exist after stream");
        let content = assistant["content"].as_str().unwrap();
        assert!(
            !content.contains("[streamed response]"),
            "assistant content must be accumulated deltas, got: {}",
            content,
        );
        assert!(!content.is_empty(), "assistant content must not be empty");
    }

    /// Fake LLM that streams a fixed sequence of deltas — used to exercise
    /// the accumulator's `push_str` loop across multiple chunks.
    struct MultiChunkLlm {
        chunks: Vec<&'static str>,
    }

    #[async_trait::async_trait]
    impl clawx_types::traits::LlmProvider for MultiChunkLlm {
        async fn complete(
            &self,
            _request: clawx_types::llm::CompletionRequest,
        ) -> clawx_types::error::Result<clawx_types::llm::LlmResponse> {
            Ok(clawx_types::llm::LlmResponse {
                content: self.chunks.join(""),
                stop_reason: clawx_types::llm::StopReason::EndTurn,
                tool_calls: vec![],
                usage: clawx_types::llm::TokenUsage::default(),
                metadata: None,
            })
        }

        async fn stream(
            &self,
            _request: clawx_types::llm::CompletionRequest,
        ) -> clawx_types::error::Result<
            std::pin::Pin<
                Box<
                    dyn futures::Stream<
                            Item = clawx_types::error::Result<
                                clawx_types::llm::LlmStreamChunk,
                            >,
                        > + Send,
                >,
            >,
        > {
            let chunks = self.chunks.clone();
            let stream = futures::stream::iter(chunks.into_iter().map(|c| {
                Ok(clawx_types::llm::LlmStreamChunk {
                    delta: c.to_string(),
                    stop_reason: None,
                    usage: None,
                })
            }));
            Ok(Box::pin(stream))
        }

        async fn test_connection(&self) -> clawx_types::error::Result<()> {
            Ok(())
        }
    }

    /// Fake LLM that yields one good chunk and then an `Err` — used to
    /// prove that mid-stream errors do NOT persist partial content.
    struct FailingLlm;

    #[async_trait::async_trait]
    impl clawx_types::traits::LlmProvider for FailingLlm {
        async fn complete(
            &self,
            _request: clawx_types::llm::CompletionRequest,
        ) -> clawx_types::error::Result<clawx_types::llm::LlmResponse> {
            unimplemented!("FailingLlm only supports stream()")
        }

        async fn stream(
            &self,
            _request: clawx_types::llm::CompletionRequest,
        ) -> clawx_types::error::Result<
            std::pin::Pin<
                Box<
                    dyn futures::Stream<
                            Item = clawx_types::error::Result<
                                clawx_types::llm::LlmStreamChunk,
                            >,
                        > + Send,
                >,
            >,
        > {
            let stream = futures::stream::iter(vec![
                Ok(clawx_types::llm::LlmStreamChunk {
                    delta: "partial".into(),
                    stop_reason: None,
                    usage: None,
                }),
                Err(clawx_types::ClawxError::LlmProvider("boom".into())),
            ]);
            Ok(Box::pin(stream))
        }

        async fn test_connection(&self) -> clawx_types::error::Result<()> {
            Ok(())
        }
    }

    /// Shared boilerplate: build an agent + empty conversation in `state`,
    /// returning the conversation ID for stream tests.
    async fn seed_agent_and_conversation(state: &AppState) -> String {
        let agent = clawx_types::agent::AgentConfig {
            id: clawx_types::ids::AgentId::new(),
            name: "t".into(),
            role: "t".into(),
            system_prompt: None,
            model_id: clawx_types::ids::ProviderId::new(),
            icon: None,
            status: clawx_types::agent::AgentStatus::Idle,
            capabilities: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_active_at: None,
        };
        let agent_id = clawx_runtime::agent_repo::create_agent(&state.runtime.db.main, &agent)
            .await
            .unwrap()
            .id;
        clawx_runtime::conversation_repo::create_conversation(
            &state.runtime.db.main,
            &agent_id.to_string(),
            None,
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn sse_accumulates_multi_chunk_stream_into_single_message() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let llm: Arc<dyn clawx_types::traits::LlmProvider> = Arc::new(MultiChunkLlm {
            chunks: vec!["Hello ", "world", "!"],
        });
        let state = make_state_with_llm(llm).await;
        let app = build_router(state.clone());
        let conv_id = seed_agent_and_conversation(&state).await;

        let body = serde_json::json!({
            "role": "user",
            "content": "ping",
            "stream": true,
        });
        let req = Request::builder()
            .method("POST")
            .uri(format!("/conversations/{}/messages", conv_id))
            .header("Authorization", "Bearer test-token-123")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = resp.into_body().collect().await.unwrap();

        let messages = clawx_runtime::conversation_repo::list_messages(
            &state.runtime.db.main,
            &conv_id,
        )
        .await
        .unwrap();
        let assistant = messages
            .iter()
            .find(|m| m["role"] == "assistant")
            .expect("assistant message must exist after stream");
        assert_eq!(
            assistant["content"].as_str().unwrap(),
            "Hello world!",
            "accumulator must concatenate all 3 deltas in order",
        );
    }

    #[tokio::test]
    async fn sse_does_not_persist_on_mid_stream_error() {
        use axum::body::Body;
        use axum::http::{Request, StatusCode};
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let state = make_state_with_llm(Arc::new(FailingLlm)).await;
        let app = build_router(state.clone());
        let conv_id = seed_agent_and_conversation(&state).await;

        let body = serde_json::json!({
            "role": "user",
            "content": "ping",
            "stream": true,
        });
        let req = Request::builder()
            .method("POST")
            .uri(format!("/conversations/{}/messages", conv_id))
            .header("Authorization", "Bearer test-token-123")
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = resp.into_body().collect().await.unwrap();

        let messages = clawx_runtime::conversation_repo::list_messages(
            &state.runtime.db.main,
            &conv_id,
        )
        .await
        .unwrap();
        assert!(
            messages.iter().all(|m| m["role"] != "assistant"),
            "should not have persisted partial content on mid-stream error, got: {:?}",
            messages,
        );
    }

    #[tokio::test]
    async fn permission_profile_for_agent() {
        let state = make_state().await;
        let router = build_router(state.clone());
        let token = "test-token-123";
        let agent_id_str = create_test_agent(&router, token).await;
        let agent_id: clawx_types::ids::AgentId = agent_id_str.parse().unwrap();

        // Create permission profile directly via DB
        let scores = clawx_types::permission::CapabilityScores::default();
        clawx_runtime::permission_repo::SqlitePermissionRepo::create_profile(
            &state.runtime.db.main,
            &agent_id,
            &scores,
        ).await.unwrap();

        // Verify profile exists via DB
        let profile = clawx_runtime::permission_repo::SqlitePermissionRepo::get_profile(
            &state.runtime.db.main,
            &agent_id,
        ).await.unwrap();
        assert!(profile.is_some());
        let profile = profile.unwrap();
        assert_eq!(profile.agent_id, agent_id);
        assert_eq!(profile.safety_incidents, 0);
    }
}
