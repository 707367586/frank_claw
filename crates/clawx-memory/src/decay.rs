//! Memory decay — exponential freshness reduction for non-pinned memories.

use chrono::Utc;
use sqlx::sqlite::SqlitePool;
use sqlx::Row;
use tracing::info;

use clawx_types::error::{ClawxError, Result};
use clawx_types::memory::DecayReport;

/// Run memory decay on all non-pinned memories.
///
/// - Applies exponential decay: `freshness *= e^(-0.05 * days_since_last_access)`
/// - Memories with freshness < 0.05 are deleted
/// - Memories with freshness < 0.2 are archived (superseded_by = "archived")
pub async fn run_memory_decay(pool: &SqlitePool) -> Result<DecayReport> {
    let now = Utc::now();
    let now_str = now.to_rfc3339();

    // Fetch all non-pinned, non-archived memories
    let rows = sqlx::query(
        "SELECT id, freshness, last_accessed_at, summary, content FROM memories WHERE is_pinned = 0 AND superseded_by IS NULL",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ClawxError::Database(e.to_string()))?;

    let mut decayed_count: u64 = 0;
    let mut archived_count: u64 = 0;
    let mut deleted_count: u64 = 0;

    for row in &rows {
        let id_str: String = row
            .try_get("id")
            .map_err(|e| ClawxError::Database(e.to_string()))?;
        let freshness: f64 = row
            .try_get("freshness")
            .map_err(|e| ClawxError::Database(e.to_string()))?;
        let last_accessed_str: String = row
            .try_get("last_accessed_at")
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        let last_accessed = chrono::DateTime::parse_from_rfc3339(&last_accessed_str)
            .map(|dt| dt.with_timezone(&Utc))
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        let days_since = (now - last_accessed).num_seconds() as f64 / 86400.0;
        let new_freshness = freshness * (-0.05 * days_since).exp();

        if new_freshness < 0.05 {
            // Delete from FTS first
            let summary: String = row
                .try_get("summary")
                .map_err(|e| ClawxError::Database(e.to_string()))?;
            let content: String = row
                .try_get("content")
                .map_err(|e| ClawxError::Database(e.to_string()))?;

            // Get rowid before delete
            let rowid_row: Option<(i64,)> =
                sqlx::query_as("SELECT rowid FROM memories WHERE id = ?")
                    .bind(&id_str)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| ClawxError::Database(e.to_string()))?;

            if let Some((rowid,)) = rowid_row {
                let _ = sqlx::query(
                    "INSERT INTO memories_fts(memories_fts, rowid, summary, content) VALUES ('delete', ?, ?, ?)",
                )
                .bind(rowid)
                .bind(&summary)
                .bind(&content)
                .execute(pool)
                .await;
            }

            sqlx::query("DELETE FROM memories WHERE id = ?")
                .bind(&id_str)
                .execute(pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
            deleted_count += 1;
        } else if new_freshness < 0.2 {
            sqlx::query(
                "UPDATE memories SET freshness = ?, superseded_by = 'archived', updated_at = ? WHERE id = ?",
            )
            .bind(new_freshness)
            .bind(&now_str)
            .bind(&id_str)
            .execute(pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;
            archived_count += 1;
            decayed_count += 1;
        } else {
            sqlx::query("UPDATE memories SET freshness = ?, updated_at = ? WHERE id = ?")
                .bind(new_freshness)
                .bind(&now_str)
                .bind(&id_str)
                .execute(pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
            decayed_count += 1;
        }
    }

    info!(
        decayed = decayed_count,
        archived = archived_count,
        deleted = deleted_count,
        "memory decay completed"
    );

    Ok(DecayReport {
        decayed_count,
        archived_count,
        deleted_count,
    })
}
