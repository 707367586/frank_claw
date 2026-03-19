//! Database initialization and migration for ClawX.
//!
//! Manages SQLite connection pools and runs schema migrations
//! for both the main database and the vault database.

use clawx_types::config::ClawxConfig;
use clawx_types::error::{ClawxError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;
use tracing::info;

/// Holds connection pools for both databases.
#[derive(Debug, Clone)]
pub struct Database {
    pub main: SqlitePool,
    pub vault: SqlitePool,
}

impl Database {
    /// Initialize databases from config, creating files and running migrations.
    pub async fn init(config: &ClawxConfig) -> Result<Self> {
        let data_dir = expand_tilde(&config.storage.data_dir);

        let main_path = data_dir.join("db/clawx.db");
        let vault_path = data_dir.join("vault/index.db");

        let main = open_pool(&main_path).await?;
        let vault = open_pool(&vault_path).await?;

        migrate_main(&main).await?;
        migrate_vault(&vault).await?;

        info!("database initialization complete");
        Ok(Self { main, vault })
    }

    /// Create an in-memory database for testing.
    pub async fn in_memory() -> Result<Self> {
        let main = open_memory_pool().await?;
        let vault = open_memory_pool().await?;
        migrate_main(&main).await?;
        migrate_vault(&vault).await?;
        Ok(Self { main, vault })
    }
}

async fn open_pool(path: &std::path::Path) -> Result<SqlitePool> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ClawxError::Internal(format!("failed to create db directory: {}", e))
        })?;
    }
    let opts = SqliteConnectOptions::from_str(&format!("sqlite:{}?mode=rwc", path.display()))
        .map_err(|e| ClawxError::Internal(format!("invalid db path: {}", e)))?
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(opts)
        .await
        .map_err(|e| ClawxError::Internal(format!("failed to open database: {}", e)))
}

async fn open_memory_pool() -> Result<SqlitePool> {
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .map_err(|e| ClawxError::Internal(format!("invalid memory db: {}", e)))?
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| ClawxError::Internal(format!("failed to open memory db: {}", e)))
}

async fn migrate_main(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql(MAIN_SCHEMA)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("main db migration failed: {}", e)))?;
    info!("main database migrated");
    Ok(())
}

async fn migrate_vault(pool: &SqlitePool) -> Result<()> {
    sqlx::raw_sql(VAULT_SCHEMA)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("vault db migration failed: {}", e)))?;
    info!("vault database migrated");
    Ok(())
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

const MAIN_SCHEMA: &str = r#"
-- Agent configuration
CREATE TABLE IF NOT EXISTS agents (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    role            TEXT NOT NULL,
    system_prompt   TEXT,
    model_id        TEXT NOT NULL,
    icon            TEXT,
    status          TEXT NOT NULL DEFAULT 'idle',
    capabilities    TEXT NOT NULL DEFAULT '[]',
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL,
    last_active_at  TEXT
);

-- Conversations
CREATE TABLE IF NOT EXISTS conversations (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id),
    title       TEXT,
    status      TEXT NOT NULL DEFAULT 'active',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Messages
CREATE TABLE IF NOT EXISTS messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    metadata        TEXT,
    created_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_messages_conversation ON messages(conversation_id, created_at);

-- Long-term memories
CREATE TABLE IF NOT EXISTS memories (
    id              TEXT PRIMARY KEY,
    scope           TEXT NOT NULL,
    agent_id        TEXT,
    kind            TEXT NOT NULL,
    summary         TEXT NOT NULL,
    content         TEXT NOT NULL,
    importance      REAL NOT NULL DEFAULT 5.0,
    freshness       REAL NOT NULL DEFAULT 1.0,
    access_count    INTEGER NOT NULL DEFAULT 0,
    is_pinned       INTEGER NOT NULL DEFAULT 0,
    source_agent_id TEXT,
    source_type     TEXT NOT NULL DEFAULT 'implicit',
    superseded_by   TEXT,
    qdrant_point_id TEXT,
    created_at      TEXT NOT NULL,
    last_accessed_at TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_memories_scope ON memories(scope, agent_id);
CREATE INDEX IF NOT EXISTS idx_memories_freshness ON memories(freshness) WHERE freshness > 0.05;
CREATE INDEX IF NOT EXISTS idx_memories_kind ON memories(kind);
CREATE INDEX IF NOT EXISTS idx_memories_active ON memories(scope, freshness)
    WHERE superseded_by IS NULL AND freshness > 0.05;

-- Memory FTS5 index
CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
    summary, content,
    content='memories', content_rowid='rowid'
);

-- Memory audit log (User Memory changes only)
CREATE TABLE IF NOT EXISTS memory_audit_log (
    id          TEXT PRIMARY KEY,
    memory_id   TEXT NOT NULL,
    agent_id    TEXT NOT NULL,
    action      TEXT NOT NULL,
    old_value   TEXT,
    new_value   TEXT,
    created_at  TEXT NOT NULL
);

-- Knowledge sources
CREATE TABLE IF NOT EXISTS knowledge_sources (
    id          TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    agent_id    TEXT,
    status      TEXT NOT NULL DEFAULT 'active',
    file_count  INTEGER NOT NULL DEFAULT 0,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    last_synced_at TEXT,
    created_at  TEXT NOT NULL
);

-- Documents
CREATE TABLE IF NOT EXISTS documents (
    id          TEXT PRIMARY KEY,
    source_id   TEXT NOT NULL REFERENCES knowledge_sources(id),
    file_path   TEXT NOT NULL,
    file_type   TEXT NOT NULL,
    file_hash   TEXT NOT NULL,
    file_size   INTEGER NOT NULL,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'pending',
    indexed_at  TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_documents_source ON documents(source_id);
CREATE INDEX IF NOT EXISTS idx_documents_hash ON documents(file_hash);

-- Chunks
CREATE TABLE IF NOT EXISTS chunks (
    id          TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES documents(id),
    chunk_index INTEGER NOT NULL,
    content     TEXT NOT NULL,
    token_count INTEGER NOT NULL,
    qdrant_point_id TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_chunks_document ON chunks(document_id, chunk_index);

-- LLM providers
CREATE TABLE IF NOT EXISTS llm_providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    type        TEXT NOT NULL,
    base_url    TEXT NOT NULL,
    model_name  TEXT NOT NULL,
    parameters  TEXT NOT NULL DEFAULT '{}',
    is_default  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Usage statistics
CREATE TABLE IF NOT EXISTS usage_stats (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id),
    provider_id TEXT NOT NULL REFERENCES llm_providers(id),
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    date        TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_usage_date ON usage_stats(date);
CREATE INDEX IF NOT EXISTS idx_usage_agent ON usage_stats(agent_id, date);
"#;

const VAULT_SCHEMA: &str = r#"
-- Vault snapshots
CREATE TABLE IF NOT EXISTS vault_snapshots (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL UNIQUE,
    agent_id    TEXT,
    task_id     TEXT,
    description TEXT,
    disk_size   INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_snapshots_created ON vault_snapshots(created_at);

-- Vault changes
CREATE TABLE IF NOT EXISTS vault_changes (
    id          TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES vault_snapshots(id),
    file_path   TEXT NOT NULL,
    change_type TEXT NOT NULL,
    old_path    TEXT,
    old_hash    TEXT,
    new_hash    TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_changes_snapshot ON vault_changes(snapshot_id);
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn in_memory_db_creates_all_tables() {
        let db = Database::in_memory().await.unwrap();

        // Verify main tables exist
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        .fetch_all(&db.main)
        .await
        .unwrap();

        let table_names: Vec<&str> = tables.iter().map(|t| t.0.as_str()).collect();
        assert!(table_names.contains(&"agents"), "missing agents table");
        assert!(table_names.contains(&"conversations"), "missing conversations table");
        assert!(table_names.contains(&"messages"), "missing messages table");
        assert!(table_names.contains(&"memories"), "missing memories table");
        assert!(table_names.contains(&"knowledge_sources"), "missing knowledge_sources table");
        assert!(table_names.contains(&"documents"), "missing documents table");
        assert!(table_names.contains(&"chunks"), "missing chunks table");
        assert!(table_names.contains(&"llm_providers"), "missing llm_providers table");
        assert!(table_names.contains(&"usage_stats"), "missing usage_stats table");
        assert!(table_names.contains(&"memory_audit_log"), "missing memory_audit_log table");

        // Verify vault tables
        let vault_tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name"
        )
        .fetch_all(&db.vault)
        .await
        .unwrap();

        let vault_names: Vec<&str> = vault_tables.iter().map(|t| t.0.as_str()).collect();
        assert!(vault_names.contains(&"vault_snapshots"), "missing vault_snapshots table");
        assert!(vault_names.contains(&"vault_changes"), "missing vault_changes table");
    }

    #[tokio::test]
    async fn can_insert_and_query_agent() {
        let db = Database::in_memory().await.unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'idle', '[]', ?, ?)"
        )
        .bind("test-agent-1")
        .bind("Test Agent")
        .bind("assistant")
        .bind("default")
        .bind(&now)
        .bind(&now)
        .execute(&db.main)
        .await
        .unwrap();

        let row: (String, String) = sqlx::query_as(
            "SELECT id, name FROM agents WHERE id = ?"
        )
        .bind("test-agent-1")
        .fetch_one(&db.main)
        .await
        .unwrap();

        assert_eq!(row.0, "test-agent-1");
        assert_eq!(row.1, "Test Agent");
    }

    #[tokio::test]
    async fn migrations_are_idempotent() {
        let db = Database::in_memory().await.unwrap();
        // Run migrations again — should not fail
        migrate_main(&db.main).await.unwrap();
        migrate_vault(&db.vault).await.unwrap();
    }
}
