//! End-to-end integration tests for ClawX core flows.
//!
//! These tests exercise the full stack: API → Runtime → Repository → SQLite,
//! using in-memory databases and stub LLM/security providers.

use std::sync::Arc;
use std::time::Instant;

use axum::body::Body;
use axum::Router;
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Test harness
// ---------------------------------------------------------------------------

async fn make_state() -> clawx_api::AppState {
    clawx_api::AppState {
        runtime: clawx_runtime::Runtime::new(
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
        control_token: "integration-test-token".to_string(),
    }
}

/// Make a state with real SQLite-backed memory service.
async fn make_state_with_real_memory() -> clawx_api::AppState {
    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let memory = Arc::new(clawx_memory::SqliteMemoryService::new(db.main.clone()));
    let working_memory = Arc::new(clawx_memory::RealWorkingMemoryManager::new(
        memory.clone(),
        clawx_memory::WorkingMemoryConfig::default(),
    ));
    clawx_api::AppState {
        runtime: clawx_runtime::Runtime::new(
            db,
            Arc::new(clawx_llm::StubLlmProvider),
            memory,
            working_memory,
            Arc::new(clawx_memory::StubMemoryExtractor),
            Arc::new(clawx_security::PermissiveSecurityGuard),
            Arc::new(clawx_vault::StubVaultService),
            Arc::new(clawx_kb::StubKnowledgeService),
            Arc::new(clawx_config::ConfigLoader::with_defaults()),
        ),
        control_token: "integration-test-token".to_string(),
    }
}

const TOKEN: &str = "integration-test-token";

async fn get(router: &Router, path: &str) -> (u16, Value) {
    let req = axum::http::Request::builder()
        .method("GET")
        .uri(path)
        .header("Authorization", format!("Bearer {}", TOKEN))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let val = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, val)
}

async fn post(router: &Router, path: &str, body: Value) -> (u16, Value) {
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(path)
        .header("Authorization", format!("Bearer {}", TOKEN))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let val = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, val)
}

async fn put(router: &Router, path: &str, body: Value) -> (u16, Value) {
    let req = axum::http::Request::builder()
        .method("PUT")
        .uri(path)
        .header("Authorization", format!("Bearer {}", TOKEN))
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let val = serde_json::from_slice(&body).unwrap_or(Value::Null);
    (status, val)
}

async fn delete(router: &Router, path: &str) -> u16 {
    let req = axum::http::Request::builder()
        .method("DELETE")
        .uri(path)
        .header("Authorization", format!("Bearer {}", TOKEN))
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    resp.status().as_u16()
}

// ===========================================================================
// Test 1: Agent full lifecycle
// ===========================================================================

#[tokio::test]
async fn agent_lifecycle_create_chat_delete() {
    let router = clawx_api::build_router(make_state().await);
    let model_id = uuid::Uuid::new_v4().to_string();

    // 1. Create agent
    let (status, agent) = post(
        &router,
        "/agents",
        json!({
            "name": "Integration Bot",
            "role": "assistant",
            "model_id": model_id,
            "system_prompt": "You are a test bot."
        }),
    )
    .await;
    assert_eq!(status, 201);
    let agent_id = agent["id"].as_str().unwrap().to_string();
    assert_eq!(agent["name"], "Integration Bot");
    assert_eq!(agent["status"], "idle");

    // 2. Get agent
    let (status, fetched) = get(&router, &format!("/agents/{}", agent_id)).await;
    assert_eq!(status, 200);
    assert_eq!(fetched["name"], "Integration Bot");

    // 3. Update agent
    let (status, updated) = put(
        &router,
        &format!("/agents/{}", agent_id),
        json!({"name": "Updated Bot"}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(updated["name"], "Updated Bot");

    // 4. Create conversation
    let (status, conv) = post(
        &router,
        "/conversations",
        json!({"agent_id": agent_id, "title": "Test Chat"}),
    )
    .await;
    assert_eq!(status, 201);
    let conv_id = conv["id"].as_str().unwrap().to_string();

    // 5. Add messages
    let (status, msg) = post(
        &router,
        &format!("/conversations/{}/messages", conv_id),
        json!({"role": "user", "content": "Hello!"}),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(msg["content"], "Hello!");

    let (status, _) = post(
        &router,
        &format!("/conversations/{}/messages", conv_id),
        json!({"role": "assistant", "content": "Hi there!"}),
    )
    .await;
    assert_eq!(status, 201);

    // 6. List messages
    let (status, msgs) = get(&router, &format!("/conversations/{}/messages", conv_id)).await;
    assert_eq!(status, 200);
    assert_eq!(msgs.as_array().unwrap().len(), 2);

    // 7. Delete conversation
    let status = delete(&router, &format!("/conversations/{}", conv_id)).await;
    assert_eq!(status, 204);

    // 8. Clone agent
    let (status, cloned) = post(
        &router,
        &format!("/agents/{}/clone", agent_id),
        json!({}),
    )
    .await;
    assert_eq!(status, 201);
    assert_eq!(cloned["name"], "Updated Bot (Copy)");

    // 9. Delete both agents
    let cloned_id = cloned["id"].as_str().unwrap();
    assert_eq!(delete(&router, &format!("/agents/{}", agent_id)).await, 204);
    assert_eq!(delete(&router, &format!("/agents/{}", cloned_id)).await, 204);

    // 10. Verify deletion
    let (status, _) = get(&router, &format!("/agents/{}", agent_id)).await;
    assert_eq!(status, 404);
}

// ===========================================================================
// Test 2: Model provider CRUD
// ===========================================================================

#[tokio::test]
async fn model_provider_lifecycle() {
    let router = clawx_api::build_router(make_state().await);

    // Create
    let (status, model) = post(
        &router,
        "/models",
        json!({
            "name": "Claude 3 Opus",
            "provider_type": "anthropic",
            "base_url": "https://api.anthropic.com",
            "model_name": "claude-3-opus-20240229",
            "parameters": {"temperature": 0.7},
            "is_default": true
        }),
    )
    .await;
    assert_eq!(status, 201);
    let model_id = model["id"].as_str().unwrap().to_string();
    assert!(model["is_default"].as_bool().unwrap());

    // List
    let (status, models) = get(&router, "/models").await;
    assert_eq!(status, 200);
    assert_eq!(models.as_array().unwrap().len(), 1);

    // Update
    let (status, updated) = put(
        &router,
        &format!("/models/{}", model_id),
        json!({"name": "Claude 3.5 Sonnet", "model_name": "claude-3-5-sonnet-20241022"}),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(updated["name"], "Claude 3.5 Sonnet");
    assert_eq!(updated["model_name"], "claude-3-5-sonnet-20241022");
    assert_eq!(updated["provider_type"], "anthropic"); // unchanged

    // Delete
    assert_eq!(delete(&router, &format!("/models/{}", model_id)).await, 204);

    // Verify gone
    let (status, _) = get(&router, &format!("/models/{}", model_id)).await;
    assert_eq!(status, 404);
}

// ===========================================================================
// Test 3: Memory system (with real SqliteMemoryService)
// ===========================================================================

#[tokio::test]
async fn memory_stats_with_real_service() {
    let router = clawx_api::build_router(make_state_with_real_memory().await);

    let (status, stats) = get(&router, "/memories/stats").await;
    assert_eq!(status, 200);
    assert_eq!(stats["total_count"], 0);
    assert_eq!(stats["agent_count"], 0);
    assert_eq!(stats["user_count"], 0);
}

#[tokio::test]
async fn memory_search_with_real_service() {
    let router = clawx_api::build_router(make_state_with_real_memory().await);

    let (status, results) = post(
        &router,
        "/memories/search",
        json!({"query_text": "test query", "top_k": 5}),
    )
    .await;
    assert_eq!(status, 200);
    assert!(results.as_array().unwrap().is_empty());
}

// ===========================================================================
// Test 4: Security — auth middleware
// ===========================================================================

#[tokio::test]
async fn auth_rejects_wrong_token() {
    let router = clawx_api::build_router(make_state().await);

    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/system/health")
        .header("Authorization", "Bearer wrong-token")
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

#[tokio::test]
async fn auth_rejects_missing_token() {
    let router = clawx_api::build_router(make_state().await);

    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/system/health")
        .body(Body::empty())
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 401);
}

// ===========================================================================
// Test 5: System health
// ===========================================================================

#[tokio::test]
async fn system_health_reports_ok() {
    let router = clawx_api::build_router(make_state().await);
    let (status, health) = get(&router, "/system/health").await;
    assert_eq!(status, 200);
    assert_eq!(health["status"], "ok");
    assert_eq!(health["version"], "0.1.0");
}

// ===========================================================================
// Test 6: Cross-entity isolation — conversations belong to agents
// ===========================================================================

#[tokio::test]
async fn conversations_isolated_by_agent() {
    let router = clawx_api::build_router(make_state().await);

    // Create two agents
    let model_id = uuid::Uuid::new_v4().to_string();
    let (_, agent_a) = post(
        &router,
        "/agents",
        json!({"name": "Agent A", "role": "assistant", "model_id": model_id}),
    )
    .await;
    let (_, agent_b) = post(
        &router,
        "/agents",
        json!({"name": "Agent B", "role": "assistant", "model_id": model_id}),
    )
    .await;
    let id_a = agent_a["id"].as_str().unwrap();
    let id_b = agent_b["id"].as_str().unwrap();

    // Create conversations for each
    let (_, _) = post(
        &router,
        "/conversations",
        json!({"agent_id": id_a, "title": "A's chat"}),
    )
    .await;
    let (_, _) = post(
        &router,
        "/conversations",
        json!({"agent_id": id_b, "title": "B's chat"}),
    )
    .await;

    // List conversations by agent — should be isolated
    let (_, convs_a) = get(
        &router,
        &format!("/conversations?agent_id={}", id_a),
    )
    .await;
    let (_, convs_b) = get(
        &router,
        &format!("/conversations?agent_id={}", id_b),
    )
    .await;

    assert_eq!(convs_a.as_array().unwrap().len(), 1);
    assert_eq!(convs_b.as_array().unwrap().len(), 1);
    assert_eq!(convs_a[0]["title"], "A's chat");
    assert_eq!(convs_b[0]["title"], "B's chat");
}

// ===========================================================================
// Test 7: Error handling for invalid inputs
// ===========================================================================

#[tokio::test]
async fn invalid_agent_id_returns_400() {
    let router = clawx_api::build_router(make_state().await);

    let (status, err) = post(
        &router,
        "/agents",
        json!({"name": "Bad", "role": "assistant", "model_id": "not-a-uuid"}),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(err["error"]["code"], "INVALID_MODEL_ID");
}

#[tokio::test]
async fn invalid_provider_type_returns_400() {
    let router = clawx_api::build_router(make_state().await);

    let (status, err) = post(
        &router,
        "/models",
        json!({
            "name": "Bad",
            "provider_type": "nonsense",
            "base_url": "https://example.com",
            "model_name": "bad"
        }),
    )
    .await;
    assert_eq!(status, 400);
    assert_eq!(err["error"]["code"], "INVALID_PROVIDER_TYPE");
}

#[tokio::test]
async fn not_found_returns_404() {
    let router = clawx_api::build_router(make_state().await);
    let fake_id = uuid::Uuid::new_v4();

    let (status, _) = get(&router, &format!("/agents/{}", fake_id)).await;
    assert_eq!(status, 404);

    let (status, _) = get(&router, &format!("/conversations/{}", fake_id)).await;
    assert_eq!(status, 404);

    let (status, _) = get(&router, &format!("/models/{}", fake_id)).await;
    assert_eq!(status, 404);
}

// ===========================================================================
// Test 8: Conversation delete cascades to messages
// ===========================================================================

#[tokio::test]
async fn delete_conversation_cascades_messages() {
    let router = clawx_api::build_router(make_state().await);
    let model_id = uuid::Uuid::new_v4().to_string();

    let (_, agent) = post(
        &router,
        "/agents",
        json!({"name": "Cascade Test", "role": "assistant", "model_id": model_id}),
    )
    .await;
    let agent_id = agent["id"].as_str().unwrap();

    let (_, conv) = post(
        &router,
        "/conversations",
        json!({"agent_id": agent_id, "title": "Will be deleted"}),
    )
    .await;
    let conv_id = conv["id"].as_str().unwrap();

    // Add messages
    post(
        &router,
        &format!("/conversations/{}/messages", conv_id),
        json!({"role": "user", "content": "msg1"}),
    )
    .await;
    post(
        &router,
        &format!("/conversations/{}/messages", conv_id),
        json!({"role": "assistant", "content": "msg2"}),
    )
    .await;

    // Verify messages exist
    let (_, msgs) = get(&router, &format!("/conversations/{}/messages", conv_id)).await;
    assert_eq!(msgs.as_array().unwrap().len(), 2);

    // Delete conversation
    assert_eq!(delete(&router, &format!("/conversations/{}", conv_id)).await, 204);

    // Messages should be gone too
    let (_, msgs) = get(&router, &format!("/conversations/{}/messages", conv_id)).await;
    assert!(msgs.as_array().unwrap().is_empty());
}

// ===========================================================================
// Helpers: states with real services
// ===========================================================================

/// Make a state with real Vault service (SQLite-backed).
async fn make_state_with_real_vault() -> clawx_api::AppState {
    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let vault = Arc::new(clawx_vault::SqliteVaultService::new(db.vault.clone()));
    clawx_api::AppState {
        runtime: clawx_runtime::Runtime::new(
            db,
            Arc::new(clawx_llm::StubLlmProvider),
            Arc::new(clawx_memory::StubMemoryService),
            Arc::new(clawx_memory::StubWorkingMemoryManager),
            Arc::new(clawx_memory::StubMemoryExtractor),
            Arc::new(clawx_security::PermissiveSecurityGuard),
            vault,
            Arc::new(clawx_kb::StubKnowledgeService),
            Arc::new(clawx_config::ConfigLoader::with_defaults()),
        ),
        control_token: "integration-test-token".to_string(),
    }
}

/// Make a state with real KB service (SQLite-backed).
async fn make_state_with_real_kb() -> clawx_api::AppState {
    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let kb = Arc::new(clawx_kb::SqliteKnowledgeService::new(db.main.clone()));
    clawx_api::AppState {
        runtime: clawx_runtime::Runtime::new(
            db,
            Arc::new(clawx_llm::StubLlmProvider),
            Arc::new(clawx_memory::StubMemoryService),
            Arc::new(clawx_memory::StubWorkingMemoryManager),
            Arc::new(clawx_memory::StubMemoryExtractor),
            Arc::new(clawx_security::PermissiveSecurityGuard),
            Arc::new(clawx_vault::StubVaultService),
            kb,
            Arc::new(clawx_config::ConfigLoader::with_defaults()),
        ),
        control_token: "integration-test-token".to_string(),
    }
}

/// Make a state with ClawxSecurityGuard (real security checks).
async fn make_state_with_real_security() -> clawx_api::AppState {
    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let security = Arc::new(clawx_security::ClawxSecurityGuard::with_network_whitelist(
        vec!["/tmp/workspace".to_string()],
        vec!["api.openai.com".to_string()],
    ));
    clawx_api::AppState {
        runtime: clawx_runtime::Runtime::new(
            db,
            Arc::new(clawx_llm::StubLlmProvider),
            Arc::new(clawx_memory::StubMemoryService),
            Arc::new(clawx_memory::StubWorkingMemoryManager),
            Arc::new(clawx_memory::StubMemoryExtractor),
            security,
            Arc::new(clawx_vault::StubVaultService),
            Arc::new(clawx_kb::StubKnowledgeService),
            Arc::new(clawx_config::ConfigLoader::with_defaults()),
        ),
        control_token: "integration-test-token".to_string(),
    }
}

// ===========================================================================
// Test 9: Vault full flow — create → list → diff → rollback
// ===========================================================================

#[tokio::test]
async fn vault_full_lifecycle() {
    let router = clawx_api::build_router(make_state_with_real_vault().await);

    // 1. List snapshots — empty
    let (status, list) = get(&router, "/vault").await;
    assert_eq!(status, 200);
    assert!(list.as_array().unwrap().is_empty());

    // 2. Create snapshot
    let (status, snap) = post(
        &router,
        "/vault",
        json!({"description": "before refactor"}),
    )
    .await;
    assert_eq!(status, 201, "create snapshot failed: {:?}", snap);
    let snap_id = snap["id"].as_str().unwrap().to_string();
    assert!(snap["label"].as_str().unwrap().starts_with("clawx-"));
    assert_eq!(snap["description"], "before refactor");

    // 3. Create another snapshot
    let (status, snap2) = post(
        &router,
        "/vault",
        json!({"description": "after refactor"}),
    )
    .await;
    assert_eq!(status, 201, "create snapshot 2 failed: {:?}", snap2);
    let snap2_id = snap2["id"].as_str().unwrap().to_string();

    // 4. List snapshots — should have 2
    let (status, list) = get(&router, "/vault").await;
    assert_eq!(status, 200);
    assert_eq!(list.as_array().unwrap().len(), 2);

    // 5. Get snapshot by ID
    let (status, fetched) = get(&router, &format!("/vault/{}", snap_id)).await;
    assert_eq!(status, 200);
    assert_eq!(fetched["description"], "before refactor");

    // 6. Diff preview
    let (status, diff) = get(&router, &format!("/vault/{}/diff", snap_id)).await;
    assert_eq!(status, 200);
    assert_eq!(diff["snapshot"]["id"], snap_id);
    assert!(diff["changes"].as_array().unwrap().is_empty()); // no changes recorded

    // 7. Rollback (returns 204 No Content)
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(&format!("/vault/{}/rollback", snap_id))
        .header("Authorization", format!("Bearer {}", TOKEN))
        .header("Content-Type", "application/json")
        .body(Body::from(b"{}".to_vec()))
        .unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status().as_u16(), 204);

    // 8. Not found for fake ID
    let fake = uuid::Uuid::new_v4();
    let (status, _) = get(&router, &format!("/vault/{}", fake)).await;
    assert_eq!(status, 404);

    let (status, _) = get(&router, &format!("/vault/{}/diff", fake)).await;
    assert_eq!(status, 404);

    let _ = snap2_id; // used
}

// ===========================================================================
// Test 10: Knowledge base full flow — add source → search
// ===========================================================================

#[tokio::test]
async fn knowledge_base_add_source_and_search() {
    let router = clawx_api::build_router(make_state_with_real_kb().await);

    // 1. Add a knowledge source
    let (status, source) = post(
        &router,
        "/knowledge/sources",
        json!({"path": "/tmp/kb-test"}),
    )
    .await;
    assert_eq!(status, 201);
    let source_id = source["id"].as_str().unwrap().to_string();
    assert_eq!(source["path"], "/tmp/kb-test");

    // 2. Search (empty — no indexed files yet)
    let (status, results) = post(
        &router,
        "/knowledge/search",
        json!({"query_text": "rust", "top_n": 5}),
    )
    .await;
    assert_eq!(status, 200);
    assert!(results.as_array().unwrap().is_empty());

    // 3. Remove source
    let status = delete(&router, &format!("/knowledge/sources/{}", source_id)).await;
    assert_eq!(status, 204);
}

// Direct service-level KB test (with actual file indexing)
#[tokio::test]
async fn knowledge_base_index_and_search_direct() {
    use clawx_types::knowledge::SearchQuery;
    use clawx_types::traits::KnowledgeService;

    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let kb = clawx_kb::SqliteKnowledgeService::new(db.main.clone());

    // Add source
    let source_id = kb.add_source("/tmp/kb-direct".into(), None).await.unwrap();

    // Create temp file and index it
    let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
    std::fs::write(
        tmp.path(),
        "Rust is a systems programming language. It focuses on safety, speed, and concurrency.",
    )
    .unwrap();

    let doc_id = kb
        .index_file(&source_id, tmp.path().to_str().unwrap())
        .await
        .unwrap();
    assert!(!doc_id.to_string().is_empty());

    // Search — should find
    let results = kb
        .search(SearchQuery {
            query_text: "safety".to_string(),
            agent_id: None,
            top_n: 5,
        })
        .await
        .unwrap();
    assert!(!results.is_empty());
    assert!(results[0].chunk.content.contains("safety"));

    // Search — no match
    let results = kb
        .search(SearchQuery {
            query_text: "quantum computing".to_string(),
            agent_id: None,
            top_n: 5,
        })
        .await
        .unwrap();
    assert!(results.is_empty());

    // Re-index same file (idempotent by hash)
    let doc_id2 = kb
        .index_file(&source_id, tmp.path().to_str().unwrap())
        .await
        .unwrap();
    assert_eq!(doc_id, doc_id2); // same hash → same doc

    // Remove source cleans up everything
    kb.remove_source(source_id).await.unwrap();

    let results = kb
        .search(SearchQuery {
            query_text: "Rust".to_string(),
            agent_id: None,
            top_n: 5,
        })
        .await
        .unwrap();
    assert!(results.is_empty());
}

// ===========================================================================
// Test 11: Security baseline — DLP, path traversal, network whitelist
// ===========================================================================

#[tokio::test]
async fn security_dlp_blocks_sensitive_data() {
    use clawx_types::security::{DataDirection, SecurityDecision};
    use clawx_types::traits::SecurityService;

    let tmp = std::fs::canonicalize("/tmp").unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let tmp_str = tmp.to_string_lossy().to_string();
    let guard = clawx_security::ClawxSecurityGuard::new(vec![tmp_str.clone()]);

    // SSH private key should be blocked
    let result = guard
        .scan_dlp(
            "-----BEGIN RSA PRIVATE KEY-----\nMIIE...",
            DataDirection::Outbound,
        )
        .await
        .unwrap();
    assert!(!result.passed, "DLP should block SSH private key");
    assert!(!result.violations.is_empty());

    // AWS access key should be blocked
    let result = guard
        .scan_dlp("AKIAIOSFODNN7EXAMPLE", DataDirection::Outbound)
        .await
        .unwrap();
    assert!(!result.passed, "DLP should block AWS key");

    // Clean content passes
    let result = guard
        .scan_dlp("Hello, this is safe content.", DataDirection::Outbound)
        .await
        .unwrap();
    assert!(result.passed, "clean content should pass DLP");

    // Path traversal blocked
    let decision = guard.check_path("/tmp/../etc/passwd").await.unwrap();
    assert!(
        matches!(decision, SecurityDecision::Deny { .. }),
        "path traversal should be blocked"
    );

    // Valid path within allowed dir
    let decision = guard.check_path(&format!("{}/workspace/file.txt", tmp_str)).await.unwrap();
    assert_eq!(decision, SecurityDecision::Allow);

    // Path outside allowed dirs
    let decision = guard.check_path("/etc/shadow").await.unwrap();
    assert!(
        matches!(decision, SecurityDecision::Deny { .. }),
        "path outside allowed dirs should be blocked"
    );
}

#[tokio::test]
async fn security_network_whitelist_enforcement() {
    use clawx_types::security::SecurityDecision;
    use clawx_types::traits::SecurityService;

    let guard = clawx_security::ClawxSecurityGuard::with_network_whitelist(
        vec![],
        vec!["api.openai.com".to_string(), "api.anthropic.com".to_string()],
    );

    // Whitelisted domains allowed
    let decision = guard
        .check_network("https://api.openai.com/v1/chat")
        .await
        .unwrap();
    assert_eq!(decision, SecurityDecision::Allow);

    let decision = guard
        .check_network("https://api.anthropic.com/v1/messages")
        .await
        .unwrap();
    assert_eq!(decision, SecurityDecision::Allow);

    // Non-whitelisted domain blocked
    let decision = guard
        .check_network("https://evil.com/steal")
        .await
        .unwrap();
    assert!(matches!(decision, SecurityDecision::Deny { .. }));

    // Private IP (SSRF) blocked
    let decision = guard
        .check_network("http://127.0.0.1:9999/admin")
        .await
        .unwrap();
    assert!(matches!(decision, SecurityDecision::Deny { .. }));

    let decision = guard
        .check_network("http://169.254.169.254/metadata")
        .await
        .unwrap();
    assert!(matches!(decision, SecurityDecision::Deny { .. }));
}

#[tokio::test]
async fn security_capability_check() {
    use clawx_types::ids::AgentId;
    use clawx_types::security::{Capability, SecurityDecision};
    use clawx_types::traits::SecurityService;

    let guard = clawx_security::ClawxSecurityGuard::new(vec![]);
    let agent_id = AgentId::new();

    // Unregistered agent — denied
    let decision = guard
        .check_capability(&agent_id, Capability::FsRead)
        .await
        .unwrap();
    assert!(matches!(decision, SecurityDecision::Deny { .. }));

    // Register capabilities
    guard.register_agent_capabilities(&agent_id, vec!["fs_read".to_string(), "llm_call".to_string()]);

    // Granted capability — allowed
    let decision = guard
        .check_capability(&agent_id, Capability::FsRead)
        .await
        .unwrap();
    assert_eq!(decision, SecurityDecision::Allow);

    // Non-granted capability — denied
    let decision = guard
        .check_capability(&agent_id, Capability::ExecShell)
        .await
        .unwrap();
    assert!(matches!(decision, SecurityDecision::Deny { .. }));
}

#[tokio::test]
async fn security_api_rejects_path_traversal_in_kb() {
    let router = clawx_api::build_router(make_state_with_real_security().await);

    // Adding a path with traversal should be denied
    let (status, err) = post(
        &router,
        "/knowledge/sources",
        json!({"path": "/tmp/workspace/../../../etc/passwd"}),
    )
    .await;
    assert_eq!(status, 403);
    assert_eq!(err["error"]["code"], "PATH_DENIED");
}

// ===========================================================================
// Test 12: Performance baselines
// ===========================================================================

#[tokio::test]
async fn perf_memory_recall_under_50ms() {
    use clawx_types::memory::MemoryQuery;
    use clawx_types::traits::MemoryService;

    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let svc = clawx_memory::SqliteMemoryService::new(db.main.clone());

    // Pre-populate with 100 memories
    for i in 0..100 {
        let entry = clawx_types::memory::MemoryEntry {
            id: clawx_types::ids::MemoryId::new(),
            scope: clawx_types::memory::MemoryScope::Agent,
            agent_id: Some(clawx_types::ids::AgentId::new()),
            kind: clawx_types::memory::MemoryKind::Fact,
            summary: format!("Fact number {} about Rust programming", i),
            content: serde_json::json!({"text": format!("Detailed content for fact {}", i)}),
            importance: 5.0 + (i as f64 * 0.05),
            freshness: 1.0,
            access_count: 0,
            is_pinned: false,
            source_agent_id: None,
            source_type: clawx_types::memory::SourceType::Implicit,
            superseded_by: None,
            qdrant_point_id: None,
            created_at: chrono::Utc::now(),
            last_accessed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        svc.store(entry).await.unwrap();
    }

    // Measure recall latency
    let start = Instant::now();
    let results = svc
        .recall(MemoryQuery {
            query_text: Some("Rust programming".to_string()),
            scope: None,
            agent_id: None,
            top_k: 5,
            include_archived: false,
            token_budget: None,
        })
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(!results.is_empty(), "should recall at least one memory");
    assert!(
        elapsed.as_millis() < 200,
        "recall should be < 200ms (P95 target), was {}ms",
        elapsed.as_millis()
    );
    // P50 target is 50ms — in-memory SQLite should be well under
    assert!(
        elapsed.as_millis() < 50,
        "in-memory recall should be < 50ms (P50 target), was {}ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn perf_kb_search_under_800ms() {
    use clawx_types::knowledge::SearchQuery;
    use clawx_types::traits::KnowledgeService;

    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let kb = clawx_kb::SqliteKnowledgeService::new(db.main.clone());

    // Add source and index multiple files
    let source_id = kb.add_source("/tmp/perf-test".into(), None).await.unwrap();

    for i in 0..20 {
        let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
        let content = format!(
            "Document {} discusses various topics including Rust, systems programming, \
             memory safety, ownership, borrowing, lifetimes, and async/await patterns. \
             It also covers error handling with Result types and pattern matching.",
            i
        );
        std::fs::write(tmp.path(), &content).unwrap();
        kb.index_file(&source_id, tmp.path().to_str().unwrap())
            .await
            .unwrap();
    }

    // Measure search latency
    let start = Instant::now();
    let results = kb
        .search(SearchQuery {
            query_text: "memory safety".to_string(),
            agent_id: None,
            top_n: 5,
        })
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(!results.is_empty(), "should find matching documents");
    assert!(
        elapsed.as_millis() < 800,
        "KB search should be < 800ms (P50 target), was {}ms",
        elapsed.as_millis()
    );
}

#[tokio::test]
async fn perf_cold_start_under_2s() {
    // Measure the time to initialize DB + create runtime
    let start = Instant::now();

    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let _runtime = clawx_runtime::Runtime::new(
        db,
        Arc::new(clawx_llm::StubLlmProvider),
        Arc::new(clawx_memory::StubMemoryService),
        Arc::new(clawx_memory::StubWorkingMemoryManager),
        Arc::new(clawx_memory::StubMemoryExtractor),
        Arc::new(clawx_security::PermissiveSecurityGuard),
        Arc::new(clawx_vault::StubVaultService),
        Arc::new(clawx_kb::StubKnowledgeService),
        Arc::new(clawx_config::ConfigLoader::with_defaults()),
    );

    let elapsed = start.elapsed();
    assert!(
        elapsed.as_secs() < 2,
        "cold start should be < 2s, was {:?}",
        elapsed
    );
}

#[tokio::test]
async fn perf_memory_footprint_baseline() {
    // Verify we can hold 10K memories in reasonable memory
    // (This is a correctness + resource usage test, not a strict memory measurement)
    use clawx_types::traits::MemoryService;

    let db = clawx_runtime::db::Database::in_memory().await.unwrap();
    let svc = clawx_memory::SqliteMemoryService::new(db.main.clone());

    // Store 1000 memories (scaled down from 10K for test speed)
    let shared_agent_id = clawx_types::ids::AgentId::new();
    for i in 0..1000 {
        let entry = clawx_types::memory::MemoryEntry {
            id: clawx_types::ids::MemoryId::new(),
            scope: if i % 3 == 0 {
                clawx_types::memory::MemoryScope::User
            } else {
                clawx_types::memory::MemoryScope::Agent
            },
            agent_id: Some(shared_agent_id),
            kind: clawx_types::memory::MemoryKind::Fact,
            summary: format!("Fact {} about Rust programming language", i),
            content: serde_json::json!({"text": format!("Rust is fast and safe, entry {}", i)}),
            importance: (i % 10) as f64,
            freshness: 1.0,
            access_count: 0,
            is_pinned: i % 50 == 0,
            source_agent_id: None,
            source_type: clawx_types::memory::SourceType::Implicit,
            superseded_by: None,
            qdrant_point_id: None,
            created_at: chrono::Utc::now(),
            last_accessed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        svc.store(entry).await.unwrap();
    }

    // Verify all stored
    let stats = svc.stats(None).await.unwrap();
    assert_eq!(stats.total_count, 1000);
    assert!(stats.agent_count > 0);
    assert!(stats.user_count > 0);
    assert!(stats.pinned_count > 0);

    // Recall still works fast with 1000 entries
    let start = Instant::now();
    let results = svc
        .recall(clawx_types::memory::MemoryQuery {
            query_text: Some("Rust".to_string()),
            scope: None,
            agent_id: None,
            top_k: 10,
            include_archived: false,
            token_budget: None,
        })
        .await
        .unwrap();
    let elapsed = start.elapsed();

    assert!(!results.is_empty(), "recall should return results for 'Rust'");
    assert!(
        elapsed.as_millis() < 200,
        "recall with 1000 entries should be < 200ms, was {}ms",
        elapsed.as_millis()
    );
}
