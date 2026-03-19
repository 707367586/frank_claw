//! Tests for SqliteMemoryService and memory decay.

use chrono::Utc;
use sqlx::sqlite::SqlitePoolOptions;

use clawx_types::ids::{AgentId, MemoryId};
use clawx_types::memory::*;
use clawx_types::pagination::Pagination;
use clawx_types::traits::MemoryService;

use crate::decay::run_memory_decay;
use crate::long_term::SqliteMemoryService;

/// Create an in-memory SQLite database with the necessary tables.
async fn setup_db() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("failed to create in-memory db");

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS memories (
            id TEXT PRIMARY KEY,
            scope TEXT NOT NULL,
            agent_id TEXT,
            kind TEXT NOT NULL,
            summary TEXT NOT NULL,
            content TEXT NOT NULL DEFAULT '{}',
            importance REAL NOT NULL DEFAULT 5.0,
            freshness REAL NOT NULL DEFAULT 1.0,
            access_count INTEGER NOT NULL DEFAULT 0,
            is_pinned INTEGER NOT NULL DEFAULT 0,
            source_agent_id TEXT,
            source_type TEXT NOT NULL DEFAULT 'implicit',
            superseded_by TEXT,
            qdrant_point_id TEXT,
            created_at TEXT NOT NULL,
            last_accessed_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )"#,
    )
    .execute(&pool)
    .await
    .expect("failed to create memories table");

    sqlx::query(
        r#"CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
            summary, content, content='memories', content_rowid='rowid'
        )"#,
    )
    .execute(&pool)
    .await
    .expect("failed to create FTS5 table");

    pool
}

fn make_entry(summary: &str, content_text: &str) -> MemoryEntry {
    MemoryEntry {
        id: MemoryId::new(),
        scope: MemoryScope::Agent,
        agent_id: Some(AgentId::new()),
        kind: MemoryKind::Fact,
        summary: summary.to_string(),
        content: serde_json::json!({ "text": content_text }),
        importance: 5.0,
        freshness: 1.0,
        access_count: 0,
        is_pinned: false,
        source_agent_id: None,
        source_type: SourceType::Implicit,
        superseded_by: None,
        qdrant_point_id: None,
        created_at: Utc::now(),
        last_accessed_at: Utc::now(),
        updated_at: Utc::now(),
    }
}

// -----------------------------------------------------------------------
// Store + Get
// -----------------------------------------------------------------------

#[tokio::test]
async fn store_and_get_returns_entry() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);
    let entry = make_entry("Rust is great", "Rust programming language");
    let id = entry.id;

    let stored_id = svc.store(entry).await.unwrap();
    assert_eq!(stored_id, id);

    let fetched = svc.get(id).await.unwrap();
    assert!(fetched.is_some());
    let fetched = fetched.unwrap();
    assert_eq!(fetched.id, id);
    assert_eq!(fetched.summary, "Rust is great");
    assert_eq!(fetched.scope, MemoryScope::Agent);
}

#[tokio::test]
async fn get_nonexistent_returns_none() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);
    let result = svc.get(MemoryId::new()).await.unwrap();
    assert!(result.is_none());
}

// -----------------------------------------------------------------------
// FTS5 Recall
// -----------------------------------------------------------------------

#[tokio::test]
async fn recall_matches_by_summary() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    svc.store(make_entry("Rust programming", "systems language"))
        .await
        .unwrap();
    svc.store(make_entry("Python scripting", "dynamic language"))
        .await
        .unwrap();
    svc.store(make_entry("Cooking recipes", "pasta and sauce"))
        .await
        .unwrap();

    let query = MemoryQuery {
        query_text: Some("Rust".to_string()),
        scope: None,
        agent_id: None,
        top_k: 5,
        include_archived: false,
        token_budget: None,
    };

    let results = svc.recall(query).await.unwrap();
    assert!(!results.is_empty());
    assert!(results[0].entry.summary.contains("Rust"));
}

#[tokio::test]
async fn recall_matches_by_content() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    svc.store(make_entry("food item", "delicious pasta recipe"))
        .await
        .unwrap();
    svc.store(make_entry("code note", "use cargo build"))
        .await
        .unwrap();

    let query = MemoryQuery {
        query_text: Some("pasta".to_string()),
        scope: None,
        agent_id: None,
        top_k: 5,
        include_archived: false,
        token_budget: None,
    };

    let results = svc.recall(query).await.unwrap();
    assert!(!results.is_empty());
    assert!(results[0].entry.summary.contains("food"));
}

#[tokio::test]
async fn recall_empty_query_returns_by_score() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    svc.store(make_entry("low importance", "stuff"))
        .await
        .unwrap();

    let mut high = make_entry("high importance", "important stuff");
    high.importance = 10.0;
    svc.store(high).await.unwrap();

    let query = MemoryQuery {
        query_text: None,
        scope: None,
        agent_id: None,
        top_k: 5,
        include_archived: false,
        token_budget: None,
    };

    let results = svc.recall(query).await.unwrap();
    assert_eq!(results.len(), 2);
    // Higher importance * freshness should come first
    assert!(results[0].entry.importance >= results[1].entry.importance);
}

#[tokio::test]
async fn recall_no_match_returns_empty() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    svc.store(make_entry("Rust programming", "systems language"))
        .await
        .unwrap();

    let query = MemoryQuery {
        query_text: Some("quantum physics".to_string()),
        scope: None,
        agent_id: None,
        top_k: 5,
        include_archived: false,
        token_budget: None,
    };

    let results = svc.recall(query).await.unwrap();
    assert!(results.is_empty());
}

// -----------------------------------------------------------------------
// Update
// -----------------------------------------------------------------------

#[tokio::test]
async fn update_changes_summary_and_importance() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);
    let entry = make_entry("original summary", "original content");
    let id = entry.id;
    svc.store(entry).await.unwrap();

    svc.update(MemoryUpdate {
        id,
        summary: Some("updated summary".to_string()),
        content: None,
        importance: Some(9.0),
        kind: None,
    })
    .await
    .unwrap();

    let fetched = svc.get(id).await.unwrap().unwrap();
    assert_eq!(fetched.summary, "updated summary");
    assert!((fetched.importance - 9.0).abs() < f64::EPSILON);
}

#[tokio::test]
async fn update_nonexistent_returns_not_found() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    let result = svc
        .update(MemoryUpdate {
            id: MemoryId::new(),
            summary: Some("nope".to_string()),
            content: None,
            importance: None,
            kind: None,
        })
        .await;

    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// Delete
// -----------------------------------------------------------------------

#[tokio::test]
async fn delete_removes_entry() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);
    let entry = make_entry("to delete", "bye");
    let id = entry.id;
    svc.store(entry).await.unwrap();

    svc.delete(id).await.unwrap();
    let fetched = svc.get(id).await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn delete_nonexistent_succeeds_silently() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);
    // Should not error
    svc.delete(MemoryId::new()).await.unwrap();
}

// -----------------------------------------------------------------------
// Toggle Pin
// -----------------------------------------------------------------------

#[tokio::test]
async fn toggle_pin_sets_pinned() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);
    let entry = make_entry("pin me", "content");
    let id = entry.id;
    svc.store(entry).await.unwrap();

    svc.toggle_pin(id, true).await.unwrap();
    let fetched = svc.get(id).await.unwrap().unwrap();
    assert!(fetched.is_pinned);

    svc.toggle_pin(id, false).await.unwrap();
    let fetched = svc.get(id).await.unwrap().unwrap();
    assert!(!fetched.is_pinned);
}

#[tokio::test]
async fn toggle_pin_nonexistent_returns_not_found() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);
    let result = svc.toggle_pin(MemoryId::new(), true).await;
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// List + Pagination
// -----------------------------------------------------------------------

#[tokio::test]
async fn list_returns_paginated_results() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    for i in 0..5 {
        svc.store(make_entry(&format!("memory {i}"), "content"))
            .await
            .unwrap();
    }

    let result = svc
        .list(
            MemoryFilter::default(),
            Pagination {
                page: 1,
                per_page: 2,
            },
        )
        .await
        .unwrap();

    assert_eq!(result.items.len(), 2);
    assert_eq!(result.total, 5);
    assert_eq!(result.page, 1);
    assert_eq!(result.per_page, 2);
}

#[tokio::test]
async fn list_with_scope_filter() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    let mut agent_entry = make_entry("agent mem", "agent stuff");
    agent_entry.scope = MemoryScope::Agent;
    svc.store(agent_entry).await.unwrap();

    let mut user_entry = make_entry("user mem", "user stuff");
    user_entry.scope = MemoryScope::User;
    user_entry.agent_id = None;
    svc.store(user_entry).await.unwrap();

    let result = svc
        .list(
            MemoryFilter {
                scope: Some(MemoryScope::User),
                ..Default::default()
            },
            Pagination::default(),
        )
        .await
        .unwrap();

    assert_eq!(result.total, 1);
    assert_eq!(result.items[0].scope, MemoryScope::User);
}

#[tokio::test]
async fn list_with_keyword_filter() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    svc.store(make_entry("Rust tips", "cargo build")).await.unwrap();
    svc.store(make_entry("Python tips", "pip install")).await.unwrap();

    let result = svc
        .list(
            MemoryFilter {
                keyword: Some("Rust".to_string()),
                ..Default::default()
            },
            Pagination::default(),
        )
        .await
        .unwrap();

    assert_eq!(result.total, 1);
    assert!(result.items[0].summary.contains("Rust"));
}

// -----------------------------------------------------------------------
// Stats
// -----------------------------------------------------------------------

#[tokio::test]
async fn stats_counts_correctly() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    let mut e1 = make_entry("agent mem", "a");
    e1.scope = MemoryScope::Agent;
    svc.store(e1).await.unwrap();

    let mut e2 = make_entry("user mem", "b");
    e2.scope = MemoryScope::User;
    e2.agent_id = None;
    svc.store(e2).await.unwrap();

    let mut e3 = make_entry("pinned mem", "c");
    e3.is_pinned = true;
    svc.store(e3).await.unwrap();

    let stats = svc.stats(None).await.unwrap();
    assert_eq!(stats.total_count, 3);
    assert_eq!(stats.agent_count, 2); // e1 + e3 (both scope=Agent)
    assert_eq!(stats.user_count, 1);
    assert_eq!(stats.pinned_count, 1);
    assert_eq!(stats.archived_count, 0);
}

// -----------------------------------------------------------------------
// Decay
// -----------------------------------------------------------------------

#[tokio::test]
async fn decay_reduces_freshness() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool.clone());

    // Create a memory with last_accessed_at 30 days ago
    let mut entry = make_entry("old memory", "stale content");
    entry.freshness = 1.0;
    entry.last_accessed_at = Utc::now() - chrono::Duration::days(30);
    svc.store(entry.clone()).await.unwrap();

    let report = run_memory_decay(&pool).await.unwrap();

    // 30 days * 0.05 = 1.5; e^(-1.5) ~ 0.223 > 0.2, so decayed but not archived
    assert!(report.decayed_count >= 1);

    let fetched = svc.get(entry.id).await.unwrap().unwrap();
    assert!(fetched.freshness < 1.0);
    assert!(fetched.freshness > 0.2);
}

#[tokio::test]
async fn decay_archives_very_stale_memories() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool.clone());

    // 40 days: freshness = 1.0 * e^(-0.05 * 40) = e^(-2.0) ~ 0.135
    // 0.135 < 0.2 so archived
    let mut entry = make_entry("stale memory", "old content");
    entry.freshness = 1.0;
    entry.last_accessed_at = Utc::now() - chrono::Duration::days(40);
    svc.store(entry.clone()).await.unwrap();

    let report = run_memory_decay(&pool).await.unwrap();
    assert_eq!(report.archived_count, 1);
}

#[tokio::test]
async fn decay_deletes_expired_memories() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool.clone());

    // freshness = 0.06, 100 days: 0.06 * e^(-5.0) ~ 0.0004 < 0.05
    let mut entry = make_entry("expired memory", "gone");
    entry.freshness = 0.06;
    entry.last_accessed_at = Utc::now() - chrono::Duration::days(100);
    svc.store(entry.clone()).await.unwrap();

    let report = run_memory_decay(&pool).await.unwrap();
    assert_eq!(report.deleted_count, 1);

    let fetched = svc.get(entry.id).await.unwrap();
    assert!(fetched.is_none());
}

#[tokio::test]
async fn decay_skips_pinned_memories() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool.clone());

    let mut entry = make_entry("pinned memory", "protected");
    entry.is_pinned = true;
    entry.freshness = 0.01; // Would be deleted if not pinned
    entry.last_accessed_at = Utc::now() - chrono::Duration::days(100);
    svc.store(entry.clone()).await.unwrap();

    let report = run_memory_decay(&pool).await.unwrap();
    assert_eq!(report.deleted_count, 0);
    assert_eq!(report.archived_count, 0);

    let fetched = svc.get(entry.id).await.unwrap();
    assert!(fetched.is_some());
}

// -----------------------------------------------------------------------
// Update then recall (FTS resync)
// -----------------------------------------------------------------------

#[tokio::test]
async fn update_resyncs_fts_index() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    let entry = make_entry("old topic", "old content");
    let id = entry.id;
    svc.store(entry).await.unwrap();

    svc.update(MemoryUpdate {
        id,
        summary: Some("quantum computing".to_string()),
        content: None,
        importance: None,
        kind: None,
    })
    .await
    .unwrap();

    // Should now find by new summary
    let results = svc
        .recall(MemoryQuery {
            query_text: Some("quantum".to_string()),
            scope: None,
            agent_id: None,
            top_k: 5,
            include_archived: false,
            token_budget: None,
        })
        .await
        .unwrap();

    assert!(!results.is_empty());
    assert_eq!(results[0].entry.id, id);
}

// -----------------------------------------------------------------------
// Edge cases
// -----------------------------------------------------------------------

#[tokio::test]
async fn store_with_special_characters_in_summary() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    let entry = make_entry("it's a \"test\" with 'quotes' & symbols", "content <html>");
    let id = entry.id;
    svc.store(entry).await.unwrap();

    let fetched = svc.get(id).await.unwrap().unwrap();
    assert!(fetched.summary.contains("\"test\""));
}

#[tokio::test]
async fn list_empty_database() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    let result = svc.list(MemoryFilter::default(), Pagination::default()).await.unwrap();
    assert_eq!(result.total, 0);
    assert!(result.items.is_empty());
}

#[tokio::test]
async fn stats_empty_database() {
    let pool = setup_db().await;
    let svc = SqliteMemoryService::new(pool);

    let stats = svc.stats(None).await.unwrap();
    assert_eq!(stats.total_count, 0);
    assert_eq!(stats.agent_count, 0);
    assert_eq!(stats.user_count, 0);
    assert_eq!(stats.pinned_count, 0);
    assert_eq!(stats.archived_count, 0);
}
