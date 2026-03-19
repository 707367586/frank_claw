//! SQLite-backed knowledge service implementation.

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::*;
use clawx_types::knowledge::*;
use clawx_types::traits::KnowledgeService;
use sqlx::SqlitePool;
use tracing::info;

use std::sync::Arc;

use crate::chunker;
use crate::hybrid;
use crate::parser;
use crate::tantivy_index::TantivyIndex;

/// Knowledge service backed by SQLite for metadata and content storage.
/// Optionally uses a Tantivy BM25 index for full-text search with hybrid RRF fusion.
#[derive(Debug, Clone)]
pub struct SqliteKnowledgeService {
    pool: SqlitePool,
    tantivy: Option<Arc<TantivyIndex>>,
}

impl SqliteKnowledgeService {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool, tantivy: None }
    }

    /// Create a service with Tantivy BM25 full-text search enabled.
    pub fn with_tantivy(pool: SqlitePool, tantivy: TantivyIndex) -> Self {
        Self {
            pool,
            tantivy: Some(Arc::new(tantivy)),
        }
    }

    /// Index a single file: parse → chunk → store in DB.
    pub async fn index_file(
        &self,
        source_id: &KnowledgeSourceId,
        file_path: &str,
    ) -> Result<DocumentId> {
        let path = std::path::Path::new(file_path);

        // Parse file
        let content = parser::parse_file(path).await?;
        let file_type = parser::detect_file_type(path);
        let file_hash = parser::content_hash(&content);
        let file_size = content.len() as u64;

        // Check if already indexed with same hash
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM documents WHERE source_id = ? AND file_path = ? AND file_hash = ?"
        )
        .bind(source_id.to_string())
        .bind(file_path)
        .bind(&file_hash)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        if let Some((id,)) = existing {
            return id.parse().map_err(|e| ClawxError::Internal(format!("invalid doc id: {}", e)));
        }

        // Delete old chunks if file was previously indexed
        // Collect old document IDs so we can also remove them from Tantivy
        let old_doc_ids: Vec<(String,)> = sqlx::query_as(
            "SELECT id FROM documents WHERE source_id = ? AND file_path = ?"
        )
        .bind(source_id.to_string())
        .bind(file_path)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        sqlx::query(
            "DELETE FROM chunks WHERE document_id IN (SELECT id FROM documents WHERE source_id = ? AND file_path = ?)"
        )
        .bind(source_id.to_string())
        .bind(file_path)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        sqlx::query("DELETE FROM documents WHERE source_id = ? AND file_path = ?")
            .bind(source_id.to_string())
            .bind(file_path)
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        // Remove old documents from Tantivy
        if let Some(ref tantivy) = self.tantivy {
            for (old_id,) in &old_doc_ids {
                tantivy.delete_by_document(old_id)?;
            }
        }

        // Create document
        let doc_id = DocumentId::new();
        let now = chrono::Utc::now().to_rfc3339();

        // Chunk the content
        let chunks = chunker::chunk_text(&content, None, None);
        let chunk_count = chunks.len() as u64;

        sqlx::query(
            "INSERT INTO documents (id, source_id, file_path, file_type, file_hash, file_size, chunk_count, status, indexed_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, 'indexed', ?, ?)"
        )
        .bind(doc_id.to_string())
        .bind(source_id.to_string())
        .bind(file_path)
        .bind(&file_type)
        .bind(&file_hash)
        .bind(file_size as i64)
        .bind(chunk_count as i64)
        .bind(&now)
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        // Insert chunks
        for chunk in &chunks {
            let chunk_id = ChunkId::new();
            sqlx::query(
                "INSERT INTO chunks (id, document_id, chunk_index, content, token_count, created_at)
                 VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(chunk_id.to_string())
            .bind(doc_id.to_string())
            .bind(chunk.index as i32)
            .bind(&chunk.content)
            .bind(chunk.token_count as i32)
            .bind(&now)
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

            // Also index in Tantivy
            if let Some(ref tantivy) = self.tantivy {
                tantivy.add_chunk(
                    &chunk_id.to_string(),
                    &doc_id.to_string(),
                    file_path,
                    &chunk.content,
                    chunk.index,
                )?;
            }
        }

        // Commit Tantivy writes
        if let Some(ref tantivy) = self.tantivy {
            tantivy.commit()?;
        }

        // Update source stats
        sqlx::query(
            "UPDATE knowledge_sources SET chunk_count = (SELECT COUNT(*) FROM chunks c JOIN documents d ON c.document_id = d.id WHERE d.source_id = ?), file_count = (SELECT COUNT(*) FROM documents WHERE source_id = ?), last_synced_at = ? WHERE id = ?"
        )
        .bind(source_id.to_string())
        .bind(source_id.to_string())
        .bind(&now)
        .bind(source_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        info!(doc_id = %doc_id, chunks = chunk_count, "indexed file");
        Ok(doc_id)
    }
}

impl SqliteKnowledgeService {
    /// Run LIKE-based search on SQLite. Used as a fallback or as one input to hybrid search.
    async fn sqlite_like_search(&self, query: &SearchQuery) -> Result<Vec<SearchResult>> {
        let pattern = format!("%{}%", query.query_text);
        // Fetch more than top_n when doing hybrid so RRF has enough candidates
        let limit = (query.top_n * 3) as i64;

        let rows: Vec<(String, String, String, String, i32, i32, String)> = if let Some(agent_id) = &query.agent_id {
            sqlx::query_as(
                "SELECT c.id, c.document_id, c.content, d.file_path, c.chunk_index, c.token_count, c.created_at
                 FROM chunks c
                 JOIN documents d ON c.document_id = d.id
                 JOIN knowledge_sources ks ON d.source_id = ks.id
                 WHERE c.content LIKE ? AND (ks.agent_id = ? OR ks.agent_id IS NULL)
                 ORDER BY c.chunk_index
                 LIMIT ?"
            )
            .bind(&pattern)
            .bind(agent_id.to_string())
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ClawxError::Internal(format!("search error: {}", e)))?
        } else {
            sqlx::query_as(
                "SELECT c.id, c.document_id, c.content, d.file_path, c.chunk_index, c.token_count, c.created_at
                 FROM chunks c
                 JOIN documents d ON c.document_id = d.id
                 WHERE c.content LIKE ?
                 ORDER BY c.chunk_index
                 LIMIT ?"
            )
            .bind(&pattern)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ClawxError::Internal(format!("search error: {}", e)))?
        };

        let results = rows.into_iter().map(|(id, doc_id, content, file_path, chunk_index, token_count, created_at)| {
            SearchResult {
                chunk: Chunk {
                    id: id.parse().unwrap_or_else(|_| ChunkId::new()),
                    document_id: doc_id.parse().unwrap_or_else(|_| DocumentId::new()),
                    chunk_index: chunk_index as u32,
                    content,
                    token_count: token_count as u32,
                    qdrant_point_id: None,
                    created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                        .map(|dt| dt.with_timezone(&chrono::Utc))
                        .unwrap_or_else(|_| chrono::Utc::now()),
                },
                document_path: file_path,
                score: 1.0, // Simple LIKE match, no real scoring
            }
        }).collect();

        Ok(results)
    }
}

#[async_trait]
impl KnowledgeService for SqliteKnowledgeService {
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>> {
        // LIKE-based SQLite search (always available)
        let like_results = self.sqlite_like_search(&query).await?;

        // If Tantivy is available, run hybrid search with RRF fusion
        if let Some(ref tantivy) = self.tantivy {
            // Fetch more candidates from each source so RRF has enough to rank
            let candidate_n = query.top_n * 3;

            // BM25 search via Tantivy
            let tantivy_hits = tantivy.search(&query.query_text, candidate_n)?;

            let bm25_ranked: Vec<(String, f64)> = tantivy_hits
                .iter()
                .map(|h| (h.chunk_id.clone(), h.bm25_score as f64))
                .collect();

            let like_ranked: Vec<(String, f64)> = like_results
                .iter()
                .map(|r| (r.chunk.id.to_string(), r.score))
                .collect();

            let fused = hybrid::rrf_fusion(bm25_ranked, like_ranked, 60.0, query.top_n);

            // Build a lookup map of chunk data from both sources
            let mut chunk_map: std::collections::HashMap<String, SearchResult> = std::collections::HashMap::new();

            // Populate from LIKE results
            for r in like_results {
                chunk_map.insert(r.chunk.id.to_string(), r);
            }

            // Populate from Tantivy hits (may have chunks not in LIKE results)
            for hit in &tantivy_hits {
                if !chunk_map.contains_key(&hit.chunk_id) {
                    chunk_map.insert(hit.chunk_id.clone(), SearchResult {
                        chunk: Chunk {
                            id: hit.chunk_id.parse().unwrap_or_else(|_| ChunkId::new()),
                            document_id: hit.document_id.parse().unwrap_or_else(|_| DocumentId::new()),
                            chunk_index: hit.chunk_index,
                            content: hit.content.clone(),
                            token_count: 0, // not stored in Tantivy
                            qdrant_point_id: None,
                            created_at: chrono::Utc::now(),
                        },
                        document_path: hit.document_path.clone(),
                        score: 0.0,
                    });
                }
            }

            // Assemble final results in RRF order
            let results: Vec<SearchResult> = fused
                .into_iter()
                .filter_map(|(chunk_id, rrf_score)| {
                    chunk_map.remove(&chunk_id).map(|mut r| {
                        r.score = rrf_score;
                        r
                    })
                })
                .collect();

            return Ok(results);
        }

        Ok(like_results)
    }

    async fn add_source(&self, path: String, agent_id: Option<AgentId>) -> Result<KnowledgeSourceId> {
        let id = KnowledgeSourceId::new();
        let now = chrono::Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT INTO knowledge_sources (id, path, agent_id, status, file_count, chunk_count, created_at)
             VALUES (?, ?, ?, 'active', 0, 0, ?)"
        )
        .bind(id.to_string())
        .bind(&path)
        .bind(agent_id.as_ref().map(|a| a.to_string()))
        .bind(&now)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        info!(source_id = %id, path = %path, "added knowledge source");
        Ok(id)
    }

    async fn remove_source(&self, source_id: KnowledgeSourceId) -> Result<()> {
        // Delete chunks → documents → source
        sqlx::query(
            "DELETE FROM chunks WHERE document_id IN (SELECT id FROM documents WHERE source_id = ?)"
        )
        .bind(source_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        sqlx::query("DELETE FROM documents WHERE source_id = ?")
            .bind(source_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        sqlx::query("DELETE FROM knowledge_sources WHERE id = ?")
            .bind(source_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Internal(format!("db error: {}", e)))?;

        info!(source_id = %source_id, "removed knowledge source");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunker;

    async fn make_pool() -> SqlitePool {
        use sqlx::sqlite::SqliteConnectOptions;
        use std::str::FromStr;
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(SqliteConnectOptions::from_str("sqlite::memory:").unwrap().foreign_keys(true))
            .await
            .unwrap();
        // Create tables
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS knowledge_sources (
                id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE, agent_id TEXT,
                status TEXT NOT NULL DEFAULT 'active', file_count INTEGER NOT NULL DEFAULT 0,
                chunk_count INTEGER NOT NULL DEFAULT 0, last_synced_at TEXT, created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY, source_id TEXT NOT NULL, file_path TEXT NOT NULL,
                file_type TEXT NOT NULL, file_hash TEXT NOT NULL, file_size INTEGER NOT NULL,
                chunk_count INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'pending',
                indexed_at TEXT, created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY, document_id TEXT NOT NULL, chunk_index INTEGER NOT NULL,
                content TEXT NOT NULL, token_count INTEGER NOT NULL, qdrant_point_id TEXT,
                created_at TEXT NOT NULL
            );"
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn add_and_remove_source() {
        let pool = make_pool().await;
        let svc = SqliteKnowledgeService::new(pool);

        let id = svc.add_source("/tmp/test".into(), None).await.unwrap();
        assert!(!id.to_string().is_empty());

        svc.remove_source(id).await.unwrap();
    }

    #[tokio::test]
    async fn index_and_search() {
        let pool = make_pool().await;
        let svc = SqliteKnowledgeService::new(pool);

        // Add source
        let source_id = svc.add_source("/tmp/kb".into(), None).await.unwrap();

        // Create a temp file and index it
        let tmp = tempfile::NamedTempFile::with_suffix(".txt").unwrap();
        std::fs::write(tmp.path(), b"Rust is a systems programming language focused on safety and performance.").unwrap();

        let doc_id = svc.index_file(&source_id, tmp.path().to_str().unwrap()).await.unwrap();
        assert!(!doc_id.to_string().is_empty());

        // Search
        let results = svc.search(SearchQuery {
            query_text: "Rust".to_string(),
            agent_id: None,
            top_n: 5,
        }).await.unwrap();

        assert!(!results.is_empty(), "should find at least one result");
        assert!(results[0].chunk.content.contains("Rust"));
    }

    #[tokio::test]
    async fn search_empty_returns_empty() {
        let pool = make_pool().await;
        let svc = SqliteKnowledgeService::new(pool);

        let results = svc.search(SearchQuery {
            query_text: "nonexistent".to_string(),
            agent_id: None,
            top_n: 5,
        }).await.unwrap();

        assert!(results.is_empty());
    }

    #[test]
    fn chunker_works_for_kb() {
        let text = "Hello world. ".repeat(200);
        let chunks = chunker::chunk_text(&text, Some(500), Some(50));
        assert!(chunks.len() > 1);
    }
}
