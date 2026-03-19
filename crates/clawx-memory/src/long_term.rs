//! SQLite-backed long-term memory service with FTS5 full-text search.

use async_trait::async_trait;
use chrono::Utc;
use sqlx::sqlite::{SqlitePool, SqliteRow};
use sqlx::Row;
use tracing::debug;

use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::*;
use clawx_types::memory::*;
use clawx_types::pagination::{PagedResult, Pagination};
use clawx_types::traits::MemoryService;

/// SQLite-backed memory service using FTS5 for full-text recall.
#[derive(Debug, Clone)]
pub struct SqliteMemoryService {
    pool: SqlitePool,
}

impl SqliteMemoryService {
    /// Create a new SqliteMemoryService with the given connection pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Sync a memory row into the FTS5 index.
    /// FTS5 with content='' (external content) requires manual insert keyed by rowid.
    async fn fts_insert(&self, memory_id: &str, summary: &str, content: &str) -> Result<()> {
        // Get the rowid of the memory row
        let row: Option<(i64,)> = sqlx::query_as("SELECT rowid FROM memories WHERE id = ?")
            .bind(memory_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        if let Some((rowid,)) = row {
            sqlx::query("INSERT INTO memories_fts(rowid, summary, content) VALUES (?, ?, ?)")
                .bind(rowid)
                .bind(summary)
                .bind(content)
                .execute(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
        }
        Ok(())
    }

    /// Remove a memory from the FTS5 index.
    async fn fts_delete(&self, memory_id: &str, old_summary: &str, old_content: &str) -> Result<()> {
        let row: Option<(i64,)> = sqlx::query_as("SELECT rowid FROM memories WHERE id = ?")
            .bind(memory_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        if let Some((rowid,)) = row {
            sqlx::query(
                "INSERT INTO memories_fts(memories_fts, rowid, summary, content) VALUES ('delete', ?, ?, ?)",
            )
            .bind(rowid)
            .bind(old_summary)
            .bind(old_content)
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;
        }
        Ok(())
    }

    /// Parse a MemoryEntry from a SqliteRow.
    fn row_to_entry(row: &SqliteRow) -> Result<MemoryEntry> {
        let id_str: String = row.try_get("id").map_err(|e| ClawxError::Database(e.to_string()))?;
        let scope_str: String = row.try_get("scope").map_err(|e| ClawxError::Database(e.to_string()))?;
        let agent_id_str: Option<String> = row.try_get("agent_id").map_err(|e| ClawxError::Database(e.to_string()))?;
        let kind_str: String = row.try_get("kind").map_err(|e| ClawxError::Database(e.to_string()))?;
        let summary: String = row.try_get("summary").map_err(|e| ClawxError::Database(e.to_string()))?;
        let content_str: String = row.try_get("content").map_err(|e| ClawxError::Database(e.to_string()))?;
        let importance: f64 = row.try_get("importance").map_err(|e| ClawxError::Database(e.to_string()))?;
        let freshness: f64 = row.try_get("freshness").map_err(|e| ClawxError::Database(e.to_string()))?;
        let access_count: i64 = row.try_get("access_count").map_err(|e| ClawxError::Database(e.to_string()))?;
        let is_pinned: bool = row.try_get("is_pinned").map_err(|e| ClawxError::Database(e.to_string()))?;
        let source_agent_id_str: Option<String> = row.try_get("source_agent_id").map_err(|e| ClawxError::Database(e.to_string()))?;
        let source_type_str: String = row.try_get("source_type").map_err(|e| ClawxError::Database(e.to_string()))?;
        let superseded_by_str: Option<String> = row.try_get("superseded_by").map_err(|e| ClawxError::Database(e.to_string()))?;
        let qdrant_point_id: Option<String> = row.try_get("qdrant_point_id").map_err(|e| ClawxError::Database(e.to_string()))?;
        let created_at_str: String = row.try_get("created_at").map_err(|e| ClawxError::Database(e.to_string()))?;
        let last_accessed_at_str: String = row.try_get("last_accessed_at").map_err(|e| ClawxError::Database(e.to_string()))?;
        let updated_at_str: String = row.try_get("updated_at").map_err(|e| ClawxError::Database(e.to_string()))?;

        let id: MemoryId = id_str
            .parse()
            .map_err(|e: uuid::Error| ClawxError::Database(e.to_string()))?;
        let scope = match scope_str.as_str() {
            "agent" => MemoryScope::Agent,
            "user" => MemoryScope::User,
            other => return Err(ClawxError::Database(format!("unknown scope: {other}"))),
        };
        let agent_id = agent_id_str
            .map(|s| s.parse::<AgentId>())
            .transpose()
            .map_err(|e: uuid::Error| ClawxError::Database(e.to_string()))?;
        let kind = match kind_str.as_str() {
            "fact" => MemoryKind::Fact,
            "preference" => MemoryKind::Preference,
            "event" => MemoryKind::Event,
            "skill" => MemoryKind::Skill,
            "contact" => MemoryKind::Contact,
            "terminology" => MemoryKind::Terminology,
            other => return Err(ClawxError::Database(format!("unknown kind: {other}"))),
        };
        let content: serde_json::Value =
            serde_json::from_str(&content_str).map_err(|e| ClawxError::Database(e.to_string()))?;
        let source_agent_id = source_agent_id_str
            .map(|s| s.parse::<AgentId>())
            .transpose()
            .map_err(|e: uuid::Error| ClawxError::Database(e.to_string()))?;
        let source_type = match source_type_str.as_str() {
            "implicit" => SourceType::Implicit,
            "explicit" => SourceType::Explicit,
            "consolidation" => SourceType::Consolidation,
            other => return Err(ClawxError::Database(format!("unknown source_type: {other}"))),
        };
        let superseded_by = superseded_by_str
            .map(|s| s.parse::<MemoryId>())
            .transpose()
            .map_err(|e: uuid::Error| ClawxError::Database(e.to_string()))?;

        let parse_dt = |s: &str| {
            chrono::DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid datetime '{s}': {e}")))
        };

        Ok(MemoryEntry {
            id,
            scope,
            agent_id,
            kind,
            summary,
            content,
            importance,
            freshness,
            access_count: access_count as u64,
            is_pinned,
            source_agent_id,
            source_type,
            superseded_by,
            qdrant_point_id,
            created_at: parse_dt(&created_at_str)?,
            last_accessed_at: parse_dt(&last_accessed_at_str)?,
            updated_at: parse_dt(&updated_at_str)?,
        })
    }
}

fn scope_to_str(scope: MemoryScope) -> &'static str {
    match scope {
        MemoryScope::Agent => "agent",
        MemoryScope::User => "user",
    }
}

fn kind_to_str(kind: MemoryKind) -> &'static str {
    match kind {
        MemoryKind::Fact => "fact",
        MemoryKind::Preference => "preference",
        MemoryKind::Event => "event",
        MemoryKind::Skill => "skill",
        MemoryKind::Contact => "contact",
        MemoryKind::Terminology => "terminology",
    }
}

fn source_type_to_str(st: SourceType) -> &'static str {
    match st {
        SourceType::Implicit => "implicit",
        SourceType::Explicit => "explicit",
        SourceType::Consolidation => "consolidation",
    }
}

#[async_trait]
impl MemoryService for SqliteMemoryService {
    async fn store(&self, entry: MemoryEntry) -> Result<MemoryId> {
        let id_str = entry.id.to_string();
        let scope_str = scope_to_str(entry.scope);
        let agent_id_str = entry.agent_id.map(|a| a.to_string());
        let kind_str = kind_to_str(entry.kind);
        let content_str = serde_json::to_string(&entry.content)
            .map_err(|e| ClawxError::Internal(e.to_string()))?;
        let source_agent_id_str = entry.source_agent_id.map(|a| a.to_string());
        let source_type_str = source_type_to_str(entry.source_type);
        let superseded_by_str = entry.superseded_by.map(|m| m.to_string());
        let created_at_str = entry.created_at.to_rfc3339();
        let last_accessed_at_str = entry.last_accessed_at.to_rfc3339();
        let updated_at_str = entry.updated_at.to_rfc3339();

        sqlx::query(
            r#"INSERT INTO memories (
                id, scope, agent_id, kind, summary, content, importance, freshness,
                access_count, is_pinned, source_agent_id, source_type, superseded_by,
                qdrant_point_id, created_at, last_accessed_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(&id_str)
        .bind(scope_str)
        .bind(&agent_id_str)
        .bind(kind_str)
        .bind(&entry.summary)
        .bind(&content_str)
        .bind(entry.importance)
        .bind(entry.freshness)
        .bind(entry.access_count as i64)
        .bind(entry.is_pinned)
        .bind(&source_agent_id_str)
        .bind(source_type_str)
        .bind(&superseded_by_str)
        .bind(&entry.qdrant_point_id)
        .bind(&created_at_str)
        .bind(&last_accessed_at_str)
        .bind(&updated_at_str)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(e.to_string()))?;

        // Sync FTS5
        self.fts_insert(&id_str, &entry.summary, &content_str).await?;

        debug!(memory_id = %id_str, "stored memory");
        Ok(entry.id)
    }

    async fn recall(&self, query: MemoryQuery) -> Result<Vec<ScoredMemory>> {
        let query_text = match &query.query_text {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => {
                // No query text: return recent memories by freshness * importance
                let mut sql = String::from(
                    "SELECT * FROM memories WHERE superseded_by IS NULL",
                );
                if !query.include_archived {
                    sql.push_str(" AND superseded_by IS NULL");
                }
                let mut bind_values: Vec<String> = Vec::new();
                if let Some(scope) = &query.scope {
                    sql.push_str(" AND scope = ?");
                    bind_values.push(scope_to_str(*scope).to_string());
                }
                if let Some(agent_id) = &query.agent_id {
                    sql.push_str(" AND agent_id = ?");
                    bind_values.push(agent_id.to_string());
                }
                sql.push_str(" ORDER BY (freshness * importance) DESC LIMIT ?");
                bind_values.push(query.top_k.to_string());

                let mut q = sqlx::query(&sql);
                for v in &bind_values {
                    q = q.bind(v);
                }
                let rows = q
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| ClawxError::Database(e.to_string()))?;

                return rows
                    .iter()
                    .map(|row| {
                        let entry = Self::row_to_entry(row)?;
                        let combined = entry.freshness * entry.importance;
                        Ok(ScoredMemory {
                            entry,
                            semantic_score: 0.0,
                            combined_score: combined,
                        })
                    })
                    .collect();
            }
        };

        // Escape FTS5 special characters by wrapping in double quotes
        let escaped_query = format!("\"{}\"", query_text.replace('"', "\"\""));

        // FTS5 MATCH query joined with memories table
        let mut sql = String::from(
            r#"SELECT m.*, bm25(memories_fts) AS rank
               FROM memories_fts
               JOIN memories m ON m.rowid = memories_fts.rowid
               WHERE memories_fts MATCH ?"#,
        );

        if !query.include_archived {
            sql.push_str(" AND m.superseded_by IS NULL");
        }
        let mut fts_bind_values: Vec<String> = vec![escaped_query];
        if let Some(scope) = &query.scope {
            sql.push_str(" AND m.scope = ?");
            fts_bind_values.push(scope_to_str(*scope).to_string());
        }
        if let Some(agent_id) = &query.agent_id {
            sql.push_str(" AND m.agent_id = ?");
            fts_bind_values.push(agent_id.to_string());
        }
        sql.push_str(" ORDER BY rank LIMIT ?");
        fts_bind_values.push(query.top_k.to_string());

        let mut q = sqlx::query(&sql);
        for v in &fts_bind_values {
            q = q.bind(v);
        }
        let rows = q
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        // Update last_accessed_at for recalled memories
        let now_str = Utc::now().to_rfc3339();
        for row in &rows {
            let id_str: String = row.try_get("id").map_err(|e| ClawxError::Database(e.to_string()))?;
            let _ = sqlx::query(
                "UPDATE memories SET access_count = access_count + 1, last_accessed_at = ? WHERE id = ?",
            )
            .bind(&now_str)
            .bind(&id_str)
            .execute(&self.pool)
            .await;
        }

        rows.iter()
            .map(|row| {
                let entry = Self::row_to_entry(row)?;
                // bm25() returns negative values; lower (more negative) = better match
                let rank: f64 = row.try_get("rank").unwrap_or(0.0);
                let semantic_score = (-rank).max(0.0); // Normalize: higher = better
                // Combined score: blend FTS rank with freshness and importance
                let combined = semantic_score * 0.6
                    + entry.freshness * 0.2
                    + (entry.importance / 10.0) * 0.2;
                Ok(ScoredMemory {
                    entry,
                    semantic_score,
                    combined_score: combined,
                })
            })
            .collect()
    }

    async fn update(&self, update: MemoryUpdate) -> Result<()> {
        let id_str = update.id.to_string();

        // Fetch old values for FTS delete
        let old_row = sqlx::query("SELECT summary, content FROM memories WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        let old_row = old_row.ok_or_else(|| ClawxError::NotFound {
            entity: "memory".to_string(),
            id: id_str.clone(),
        })?;

        let old_summary: String = old_row
            .try_get("summary")
            .map_err(|e| ClawxError::Database(e.to_string()))?;
        let old_content: String = old_row
            .try_get("content")
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        // FTS delete old entry
        self.fts_delete(&id_str, &old_summary, &old_content).await?;

        // Build dynamic UPDATE
        let mut sets = vec!["updated_at = ?".to_string()];
        let now_str = Utc::now().to_rfc3339();

        let new_summary = update.summary.clone();
        let new_content = update
            .content
            .as_ref()
            .map(|c| serde_json::to_string(c).unwrap_or_default());
        let new_importance = update.importance;
        let new_kind = update.kind;

        if new_summary.is_some() {
            sets.push("summary = ?".to_string());
        }
        if new_content.is_some() {
            sets.push("content = ?".to_string());
        }
        if new_importance.is_some() {
            sets.push("importance = ?".to_string());
        }
        if new_kind.is_some() {
            sets.push("kind = ?".to_string());
        }

        let _sql = format!("UPDATE memories SET {} WHERE id = ?", sets.join(", "));

        // We need to build the query dynamically; use a simple approach
        // Since sqlx doesn't support truly dynamic binding easily, we'll use
        // individual UPDATE statements for simplicity.
        if let Some(ref summary) = new_summary {
            sqlx::query("UPDATE memories SET summary = ? WHERE id = ?")
                .bind(summary)
                .bind(&id_str)
                .execute(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
        }
        if let Some(ref content) = new_content {
            sqlx::query("UPDATE memories SET content = ? WHERE id = ?")
                .bind(content)
                .bind(&id_str)
                .execute(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
        }
        if let Some(importance) = new_importance {
            sqlx::query("UPDATE memories SET importance = ? WHERE id = ?")
                .bind(importance)
                .bind(&id_str)
                .execute(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
        }
        if let Some(kind) = new_kind {
            sqlx::query("UPDATE memories SET kind = ? WHERE id = ?")
                .bind(kind_to_str(kind))
                .bind(&id_str)
                .execute(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
        }
        sqlx::query("UPDATE memories SET updated_at = ? WHERE id = ?")
            .bind(&now_str)
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        // FTS re-insert with new values
        let final_summary = new_summary.unwrap_or(old_summary);
        let final_content = new_content.unwrap_or(old_content);
        self.fts_insert(&id_str, &final_summary, &final_content).await?;

        debug!(memory_id = %id_str, "updated memory");
        Ok(())
    }

    async fn delete(&self, id: MemoryId) -> Result<()> {
        let id_str = id.to_string();

        // Fetch for FTS delete
        let old_row = sqlx::query("SELECT summary, content FROM memories WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        if let Some(row) = old_row {
            let old_summary: String = row
                .try_get("summary")
                .map_err(|e| ClawxError::Database(e.to_string()))?;
            let old_content: String = row
                .try_get("content")
                .map_err(|e| ClawxError::Database(e.to_string()))?;
            self.fts_delete(&id_str, &old_summary, &old_content).await?;
        }

        sqlx::query("DELETE FROM memories WHERE id = ?")
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        debug!(memory_id = %id_str, "deleted memory");
        Ok(())
    }

    async fn toggle_pin(&self, id: MemoryId, pinned: bool) -> Result<()> {
        let id_str = id.to_string();
        let now_str = Utc::now().to_rfc3339();

        let result = sqlx::query("UPDATE memories SET is_pinned = ?, updated_at = ? WHERE id = ?")
            .bind(pinned)
            .bind(&now_str)
            .bind(&id_str)
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        if result.rows_affected() == 0 {
            return Err(ClawxError::NotFound {
                entity: "memory".to_string(),
                id: id_str,
            });
        }

        debug!(memory_id = %id_str, pinned, "toggled pin");
        Ok(())
    }

    async fn get(&self, id: MemoryId) -> Result<Option<MemoryEntry>> {
        let id_str = id.to_string();

        let row = sqlx::query("SELECT * FROM memories WHERE id = ?")
            .bind(&id_str)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        match row {
            Some(ref r) => Ok(Some(Self::row_to_entry(r)?)),
            None => Ok(None),
        }
    }

    async fn list(
        &self,
        filter: MemoryFilter,
        pagination: Pagination,
    ) -> Result<PagedResult<MemoryEntry>> {
        let mut where_clauses = Vec::new();
        let mut bind_values: Vec<String> = Vec::new();

        if !filter.include_archived {
            where_clauses.push("superseded_by IS NULL".to_string());
        }
        if let Some(scope) = &filter.scope {
            where_clauses.push("scope = ?".to_string());
            bind_values.push(scope_to_str(*scope).to_string());
        }
        if let Some(agent_id) = &filter.agent_id {
            where_clauses.push("agent_id = ?".to_string());
            bind_values.push(agent_id.to_string());
        }
        if let Some(kind) = &filter.kind {
            where_clauses.push("kind = ?".to_string());
            bind_values.push(kind_to_str(*kind).to_string());
        }
        if let Some(keyword) = &filter.keyword {
            where_clauses.push("(summary LIKE '%' || ? || '%' OR content LIKE '%' || ? || '%')".to_string());
            bind_values.push(keyword.clone());
            bind_values.push(keyword.clone());
        }
        if let Some(min_imp) = filter.min_importance {
            where_clauses.push("importance >= ?".to_string());
            bind_values.push(min_imp.to_string());
        }

        let where_sql = if where_clauses.is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_clauses.join(" AND "))
        };

        // Count total
        let count_sql = format!("SELECT COUNT(*) as cnt FROM memories{}", where_sql);
        let mut count_query = sqlx::query(&count_sql);
        for val in &bind_values {
            count_query = count_query.bind(val);
        }
        let count_row = count_query
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;
        let total: i64 = count_row
            .try_get("cnt")
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        let offset = (pagination.page.saturating_sub(1)) * pagination.per_page;
        let select_sql = format!(
            "SELECT * FROM memories{} ORDER BY updated_at DESC LIMIT ? OFFSET ?",
            where_sql
        );

        let mut select_query = sqlx::query(&select_sql);
        for val in &bind_values {
            select_query = select_query.bind(val);
        }
        select_query = select_query
            .bind(pagination.per_page as i64)
            .bind(offset as i64);
        let rows = select_query
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(e.to_string()))?;

        let items: Result<Vec<MemoryEntry>> = rows.iter().map(Self::row_to_entry).collect();

        Ok(PagedResult {
            items: items?,
            total: total as u64,
            page: pagination.page,
            per_page: pagination.per_page,
        })
    }

    async fn stats(&self, agent_id: Option<AgentId>) -> Result<MemoryStats> {
        let has_agent = agent_id.is_some();
        let agent_str = agent_id.map(|a| a.to_string());

        async fn count_with_filter(
            pool: &sqlx::SqlitePool,
            base_sql: &str,
            agent_str: &Option<String>,
            has_agent: bool,
        ) -> Result<i64> {
            let sql = if has_agent {
                format!("{} AND agent_id = ?", base_sql)
            } else {
                base_sql.to_string()
            };
            let mut query = sqlx::query_as::<_, (i64,)>(&sql);
            if let Some(aid) = agent_str {
                query = query.bind(aid);
            }
            let row = query
                .fetch_one(pool)
                .await
                .map_err(|e| ClawxError::Database(e.to_string()))?;
            Ok(row.0)
        }

        let total = count_with_filter(
            &self.pool, "SELECT COUNT(*) FROM memories WHERE 1=1", &agent_str, has_agent,
        ).await?;
        let agent_count = count_with_filter(
            &self.pool, "SELECT COUNT(*) FROM memories WHERE scope = 'agent'", &agent_str, has_agent,
        ).await?;
        let user_count = count_with_filter(
            &self.pool, "SELECT COUNT(*) FROM memories WHERE scope = 'user'", &agent_str, has_agent,
        ).await?;
        let pinned_count = count_with_filter(
            &self.pool, "SELECT COUNT(*) FROM memories WHERE is_pinned = 1", &agent_str, has_agent,
        ).await?;
        let archived_count = count_with_filter(
            &self.pool, "SELECT COUNT(*) FROM memories WHERE superseded_by IS NOT NULL", &agent_str, has_agent,
        ).await?;

        Ok(MemoryStats {
            total_count: total as u64,
            agent_count: agent_count as u64,
            user_count: user_count as u64,
            pinned_count: pinned_count as u64,
            archived_count: archived_count as u64,
        })
    }
}

