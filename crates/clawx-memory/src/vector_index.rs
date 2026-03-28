//! Vector-based memory index using QdrantStore for semantic search.
//!
//! Wraps [`QdrantStore`] from `clawx-kb` to provide memory-specific
//! vector storage, retrieval, and hybrid recall (FTS5 + vector via RRF fusion).

use clawx_kb::qdrant::{QdrantStore, VectorSearchResult};
use clawx_kb::reciprocal_rank_fusion;
use clawx_types::error::Result;

/// A single memory search result from the vector index.
#[derive(Debug, Clone)]
pub struct MemorySearchResult {
    /// The memory ID stored in the vector index.
    pub memory_id: String,
    /// Cosine similarity score (higher = more similar).
    pub score: f32,
    /// The text content that was indexed.
    pub content: String,
}

/// Memory-specific vector index backed by a [`QdrantStore`].
///
/// Provides embedding-based indexing, semantic search, and hybrid
/// recall that fuses FTS5 keyword results with vector similarity
/// using Reciprocal Rank Fusion (RRF).
pub struct VectorMemoryIndex {
    store: QdrantStore,
}

impl VectorMemoryIndex {
    /// Create a new vector memory index wrapping the given store.
    pub fn new(store: QdrantStore) -> Self {
        Self { store }
    }

    /// Embed and upsert a memory's content into the vector store.
    ///
    /// If a memory with the same `memory_id` already exists it is overwritten
    /// (upsert semantics), so callers can safely re-index after updates.
    pub async fn index_memory(&self, memory_id: &str, content: &str) -> Result<()> {
        self.store
            .upsert_text(
                memory_id,
                content,
                serde_json::json!({ "memory_id": memory_id }),
            )
            .await
    }

    /// Search the vector index for memories semantically similar to `query`.
    ///
    /// Returns up to `top_k` results sorted by descending similarity score.
    pub async fn search_memories(
        &self,
        query: &str,
        top_k: usize,
    ) -> Result<Vec<MemorySearchResult>> {
        let results: Vec<VectorSearchResult> = self.store.search(query, top_k).await?;
        Ok(results
            .into_iter()
            .map(|r| MemorySearchResult {
                memory_id: r.point_id,
                score: r.score,
                content: r.content,
            })
            .collect())
    }

    /// Remove a memory from the vector index.
    ///
    /// Returns `true` if the memory existed and was removed.
    pub async fn delete_memory(&self, memory_id: &str) -> Result<bool> {
        self.store.delete(memory_id).await
    }

    /// Combine FTS5 keyword results with vector similarity search using RRF fusion.
    ///
    /// `fts_results` is a ranked list of `(memory_id, score)` pairs from FTS5.
    /// The method performs a vector search for `query`, then fuses both ranked
    /// lists with Reciprocal Rank Fusion (k=60) to produce a single ordering.
    ///
    /// Returns `(memory_id, rrf_score)` pairs sorted by descending fused score,
    /// truncated to `top_k`.
    pub async fn hybrid_recall(
        &self,
        fts_results: &[(String, f32)],
        query: &str,
        top_k: usize,
    ) -> Result<Vec<(String, f32)>> {
        // Vector search with a generous limit so RRF has enough candidates
        let vector_results = self.store.search(query, top_k * 2).await?;
        let vector_ranked: Vec<(String, f32)> = vector_results
            .into_iter()
            .map(|r| (r.point_id, r.score))
            .collect();

        let lists = vec![fts_results.to_vec(), vector_ranked];
        let mut fused = reciprocal_rank_fusion(&lists, 60.0);
        fused.truncate(top_k);
        Ok(fused)
    }

    /// Return the number of indexed memory vectors.
    pub async fn count(&self) -> usize {
        self.store.count().await
    }
}
