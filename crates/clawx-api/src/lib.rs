//! RESTful API server for ClawX.
//!
//! Exposes agent runtime operations over HTTP/UDS using Axum.
//! All routes return 501 Not Implemented in the skeleton phase.

mod routes;
mod middleware;

use std::sync::Arc;

use axum::Router;
use clawx_runtime::Runtime;
use tracing::info;

/// Application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    pub runtime: Runtime,
    pub control_token: String,
}

/// Build the complete API router with all route groups.
pub fn build_router(state: AppState) -> Router {
    let shared = Arc::new(state);
    Router::new()
        .nest("/agents", routes::agents::router())
        .nest("/conversations", routes::conversations::router())
        .nest("/memories", routes::memories::router())
        .nest("/knowledge", routes::knowledge::router())
        .nest("/vault", routes::vault::router())
        .nest("/models", routes::models::router())
        .nest("/system", routes::system::router())
        .layer(axum::middleware::from_fn_with_state(
            shared.clone(),
            middleware::auth::require_token,
        ))
        .with_state(shared)
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
        AppState {
            runtime: Runtime::new(
                clawx_runtime::db::Database::in_memory().await.unwrap(),
                Arc::new(clawx_llm::StubLlmProvider),
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
}
