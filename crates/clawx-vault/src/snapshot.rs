//! SQLite-backed vault service for snapshot management.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::*;
use clawx_types::traits::VaultService;
use clawx_types::vault::*;
use sqlx::SqlitePool;
use std::str::FromStr;
use tracing::info;

/// Row returned by SELECT on vault_snapshots.
#[derive(Debug, sqlx::FromRow)]
struct SnapshotRow {
    id: String,
    label: String,
    agent_id: Option<String>,
    task_id: Option<String>,
    description: Option<String>,
    disk_size: i64,
    created_at: String,
}

impl TryFrom<SnapshotRow> for VaultSnapshot {
    type Error = ClawxError;

    fn try_from(row: SnapshotRow) -> Result<Self> {
        Ok(VaultSnapshot {
            id: SnapshotId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid snapshot id: {}", e)))?,
            label: row.label,
            agent_id: row
                .agent_id
                .map(|s| AgentId::from_str(&s))
                .transpose()
                .map_err(|e| ClawxError::Database(format!("invalid agent id: {}", e)))?,
            task_id: row
                .task_id
                .map(|s| TaskId::from_str(&s))
                .transpose()
                .map_err(|e| ClawxError::Database(format!("invalid task id: {}", e)))?,
            description: row.description,
            disk_size: row.disk_size as u64,
            created_at: DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
        })
    }
}

/// Row returned by SELECT on vault_changes.
#[derive(Debug, sqlx::FromRow)]
struct ChangeRow {
    id: String,
    snapshot_id: String,
    file_path: String,
    change_type: String,
    old_path: Option<String>,
    old_hash: Option<String>,
    new_hash: Option<String>,
    created_at: String,
}

impl TryFrom<ChangeRow> for VaultChange {
    type Error = ClawxError;

    fn try_from(row: ChangeRow) -> Result<Self> {
        Ok(VaultChange {
            id: row.id,
            snapshot_id: SnapshotId::from_str(&row.snapshot_id)
                .map_err(|e| ClawxError::Database(format!("invalid snapshot id: {}", e)))?,
            file_path: row.file_path,
            change_type: ChangeType::from_str(&row.change_type)
                .map_err(|e| ClawxError::Database(format!("invalid change type: {}", e)))?,
            old_path: row.old_path,
            old_hash: row.old_hash,
            new_hash: row.new_hash,
            created_at: DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
        })
    }
}

/// SQLite-backed vault service.
#[derive(Debug, Clone)]
pub struct SqliteVaultService {
    pool: SqlitePool,
}

impl SqliteVaultService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl VaultService for SqliteVaultService {
    async fn create_snapshot(
        &self,
        agent_id: Option<AgentId>,
        task_id: Option<TaskId>,
        description: Option<String>,
    ) -> Result<VaultSnapshot> {
        let id = SnapshotId::new();
        let now = Utc::now();
        let timestamp = now.format("%Y%m%d%H%M%S").to_string();
        let agent_str = agent_id
            .as_ref()
            .map(|a| a.to_string())
            .unwrap_or_else(|| "none".to_string());
        let task_str = task_id
            .as_ref()
            .map(|t| t.to_string())
            .unwrap_or_else(|| "none".to_string());
        let short_id = &id.to_string()[..8]; // first 8 chars of UUID for uniqueness
        let label = format!("clawx-{}-{}-{}-{}", agent_str, task_str, timestamp, short_id);

        let id_str = id.to_string();
        let agent_id_str = agent_id.map(|a| a.to_string());
        let task_id_str = task_id.map(|t| t.to_string());
        let created_at = now.to_rfc3339();

        sqlx::query(
            "INSERT INTO vault_snapshots (id, label, agent_id, task_id, description, disk_size, created_at)
             VALUES (?, ?, ?, ?, ?, 0, ?)",
        )
        .bind(&id_str)
        .bind(&label)
        .bind(&agent_id_str)
        .bind(&task_id_str)
        .bind(&description)
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("create snapshot: {}", e)))?;

        info!(snapshot_id = %id, label = %label, "vault snapshot created");

        Ok(VaultSnapshot {
            id,
            label,
            agent_id: agent_id_str
                .map(|s| AgentId::from_str(&s).unwrap()),
            task_id: task_id_str
                .map(|s| TaskId::from_str(&s).unwrap()),
            description,
            disk_size: 0,
            created_at: now,
        })
    }

    async fn list_snapshots(&self) -> Result<Vec<VaultSnapshot>> {
        let rows: Vec<SnapshotRow> =
            sqlx::query_as("SELECT * FROM vault_snapshots ORDER BY created_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(format!("list snapshots: {}", e)))?;

        rows.into_iter().map(VaultSnapshot::try_from).collect()
    }

    async fn diff_preview(&self, snapshot_id: SnapshotId) -> Result<DiffPreview> {
        let snap_row: Option<SnapshotRow> =
            sqlx::query_as("SELECT * FROM vault_snapshots WHERE id = ?")
                .bind(snapshot_id.to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(format!("get snapshot: {}", e)))?;

        let snapshot = snap_row
            .ok_or_else(|| ClawxError::NotFound {
                entity: "snapshot".into(),
                id: snapshot_id.to_string(),
            })
            .and_then(VaultSnapshot::try_from)?;

        let change_rows: Vec<ChangeRow> =
            sqlx::query_as("SELECT * FROM vault_changes WHERE snapshot_id = ? ORDER BY created_at")
                .bind(snapshot_id.to_string())
                .fetch_all(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(format!("list changes: {}", e)))?;

        let changes: Vec<VaultChange> = change_rows
            .into_iter()
            .map(VaultChange::try_from)
            .collect::<Result<_>>()?;

        Ok(DiffPreview { snapshot, changes })
    }

    async fn rollback(&self, snapshot_id: SnapshotId) -> Result<()> {
        // Verify snapshot exists
        let exists: Option<(String,)> =
            sqlx::query_as("SELECT id FROM vault_snapshots WHERE id = ?")
                .bind(snapshot_id.to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(format!("check snapshot: {}", e)))?;

        if exists.is_none() {
            return Err(ClawxError::NotFound {
                entity: "snapshot".into(),
                id: snapshot_id.to_string(),
            });
        }

        // TODO: actual file-level rollback from blob backups
        info!(snapshot_id = %snapshot_id, "vault rollback executed (DB-only, file restore not yet implemented)");

        Ok(())
    }
}

/// Delete snapshots older than `retention_days`.
pub async fn cleanup_old_snapshots(pool: &SqlitePool, retention_days: u32) -> Result<u64> {
    let cutoff = Utc::now() - chrono::Duration::days(retention_days as i64);
    let cutoff_str = cutoff.to_rfc3339();

    // Delete changes for old snapshots first (FK constraint)
    sqlx::query(
        "DELETE FROM vault_changes WHERE snapshot_id IN (SELECT id FROM vault_snapshots WHERE created_at < ?)",
    )
    .bind(&cutoff_str)
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("cleanup changes: {}", e)))?;

    let result = sqlx::query("DELETE FROM vault_snapshots WHERE created_at < ?")
        .bind(&cutoff_str)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("cleanup snapshots: {}", e)))?;

    let deleted = result.rows_affected();
    if deleted > 0 {
        info!(deleted, retention_days, "cleaned up old vault snapshots");
    }
    Ok(deleted)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();

        sqlx::raw_sql(
            r#"
            CREATE TABLE IF NOT EXISTS vault_snapshots (
                id          TEXT PRIMARY KEY,
                label       TEXT NOT NULL UNIQUE,
                agent_id    TEXT,
                task_id     TEXT,
                description TEXT,
                disk_size   INTEGER NOT NULL DEFAULT 0,
                created_at  TEXT NOT NULL
            );
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
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();

        pool
    }

    #[tokio::test]
    async fn create_and_list_snapshots() {
        let pool = setup_pool().await;
        let svc = SqliteVaultService::new(pool);

        let snap = svc
            .create_snapshot(None, None, Some("test snapshot".into()))
            .await
            .unwrap();

        assert!(snap.label.starts_with("clawx-"));
        assert_eq!(snap.description, Some("test snapshot".to_string()));
        assert_eq!(snap.disk_size, 0);

        let list = svc.list_snapshots().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, snap.id);
    }

    #[tokio::test]
    async fn create_snapshot_with_agent_and_task() {
        let pool = setup_pool().await;
        let svc = SqliteVaultService::new(pool);

        let agent_id = AgentId::new();
        let task_id = TaskId::new();
        let snap = svc
            .create_snapshot(Some(agent_id), Some(task_id), None)
            .await
            .unwrap();

        assert_eq!(snap.agent_id, Some(agent_id));
        assert_eq!(snap.task_id, Some(task_id));
        assert!(snap.label.contains(&agent_id.to_string()));
    }

    #[tokio::test]
    async fn diff_preview_with_changes() {
        let pool = setup_pool().await;
        let svc = SqliteVaultService::new(pool.clone());

        let snap = svc
            .create_snapshot(None, None, Some("diff test".into()))
            .await
            .unwrap();

        // Insert a change manually
        let change_id = uuid::Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO vault_changes (id, snapshot_id, file_path, change_type, created_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&change_id)
        .bind(snap.id.to_string())
        .bind("src/main.rs")
        .bind("modified")
        .bind(&now)
        .execute(&pool)
        .await
        .unwrap();

        let preview = svc.diff_preview(snap.id).await.unwrap();
        assert_eq!(preview.snapshot.id, snap.id);
        assert_eq!(preview.changes.len(), 1);
        assert_eq!(preview.changes[0].file_path, "src/main.rs");
        assert_eq!(preview.changes[0].change_type, ChangeType::Modified);
    }

    #[tokio::test]
    async fn diff_preview_not_found() {
        let pool = setup_pool().await;
        let svc = SqliteVaultService::new(pool);

        let result = svc.diff_preview(SnapshotId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn rollback_not_found() {
        let pool = setup_pool().await;
        let svc = SqliteVaultService::new(pool);

        let result = svc.rollback(SnapshotId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn rollback_existing_snapshot() {
        let pool = setup_pool().await;
        let svc = SqliteVaultService::new(pool);

        let snap = svc
            .create_snapshot(None, None, Some("rollback test".into()))
            .await
            .unwrap();

        // Should succeed (no-op for now)
        svc.rollback(snap.id).await.unwrap();
    }

    #[tokio::test]
    async fn cleanup_old_snapshots_removes_expired() {
        let pool = setup_pool().await;

        // Insert an old snapshot with a past timestamp
        let old_time = (Utc::now() - chrono::Duration::days(10)).to_rfc3339();
        let old_id = SnapshotId::new().to_string();
        sqlx::query(
            "INSERT INTO vault_snapshots (id, label, disk_size, created_at) VALUES (?, ?, 0, ?)",
        )
        .bind(&old_id)
        .bind("clawx-old-snapshot")
        .bind(&old_time)
        .execute(&pool)
        .await
        .unwrap();

        // Insert a recent snapshot
        let svc = SqliteVaultService::new(pool.clone());
        svc.create_snapshot(None, None, Some("recent".into()))
            .await
            .unwrap();

        let deleted = cleanup_old_snapshots(&pool, 7).await.unwrap();
        assert_eq!(deleted, 1);

        let remaining = svc.list_snapshots().await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].description, Some("recent".to_string()));
    }

    #[tokio::test]
    async fn cleanup_no_old_snapshots() {
        let pool = setup_pool().await;
        let svc = SqliteVaultService::new(pool.clone());
        svc.create_snapshot(None, None, None).await.unwrap();

        let deleted = cleanup_old_snapshots(&pool, 7).await.unwrap();
        assert_eq!(deleted, 0);
    }
}
