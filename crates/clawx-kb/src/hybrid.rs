//! Reciprocal Rank Fusion (RRF) for combining search results from multiple sources.
//!
//! Also provides `HybridSearchEngine` which combines QdrantStore vector search
//! with BM25 keyword results using RRF fusion.

use std::collections::HashMap;

use clawx_types::error::Result;

use crate::qdrant::QdrantStore;

/// Combine two ranked result lists using Reciprocal Rank Fusion.
///
/// Each input is a list of `(chunk_id, score)` pairs ordered by descending score.
/// The RRF formula: `rrf_score = sum(1 / (k + rank_i))` where `rank_i` is the
/// 1-based rank in each result list.
///
/// Returns `(chunk_id, rrf_score)` pairs sorted by descending RRF score, limited to `top_n`.
pub fn rrf_fusion(
    results_a: Vec<(String, f64)>,
    results_b: Vec<(String, f64)>,
    k: f64,
    top_n: usize,
) -> Vec<(String, f64)> {
    let mut scores: HashMap<String, f64> = HashMap::new();

    for (rank_0, (chunk_id, _score)) in results_a.into_iter().enumerate() {
        let rank = (rank_0 + 1) as f64; // 1-based
        *scores.entry(chunk_id).or_insert(0.0) += 1.0 / (k + rank);
    }

    for (rank_0, (chunk_id, _score)) in results_b.into_iter().enumerate() {
        let rank = (rank_0 + 1) as f64;
        *scores.entry(chunk_id).or_insert(0.0) += 1.0 / (k + rank);
    }

    let mut fused: Vec<(String, f64)> = scores.into_iter().collect();
    fused.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    fused.truncate(top_n);
    fused
}

/// A hybrid search result combining vector and BM25 scores.
#[derive(Debug, Clone)]
pub struct HybridSearchResult {
    /// The chunk identifier.
    pub chunk_id: String,
    /// The fused RRF score.
    pub score: f64,
    /// The text content (populated from vector search when available).
    pub content: Option<String>,
    /// Additional metadata from vector search.
    pub metadata: Option<serde_json::Value>,
}

/// Hybrid search engine that combines QdrantStore vector search with BM25 keyword
/// results using Reciprocal Rank Fusion (RRF).
pub struct HybridSearchEngine {
    qdrant: QdrantStore,
    /// RRF constant `k` — higher values reduce the impact of high-ranked items.
    rrf_k: f64,
}

impl HybridSearchEngine {
    /// Create a new hybrid search engine wrapping a QdrantStore.
    pub fn new(qdrant: QdrantStore) -> Self {
        Self { qdrant, rrf_k: 60.0 }
    }

    /// Create with a custom RRF `k` parameter.
    pub fn with_rrf_k(qdrant: QdrantStore, rrf_k: f64) -> Self {
        Self { qdrant, rrf_k }
    }

    /// Run hybrid search: combine vector search results with provided BM25 results
    /// using RRF fusion.
    ///
    /// `bm25_results` is a list of `(chunk_id, bm25_score)` pairs from an external
    /// BM25 source (e.g., Tantivy), ordered by descending score.
    pub async fn hybrid_search(
        &self,
        query: &str,
        bm25_results: Vec<(String, f64)>,
        top_k: usize,
    ) -> Result<Vec<HybridSearchResult>> {
        // Run vector search — fetch more candidates so RRF has enough to rank
        let candidate_k = top_k * 3;
        let vector_results = self.qdrant.search(query, candidate_k).await?;

        // Convert vector results to ranked list
        let vector_ranked: Vec<(String, f64)> = vector_results
            .iter()
            .map(|r| (r.point_id.clone(), r.score as f64))
            .collect();

        // Fuse with RRF
        let fused = rrf_fusion(vector_ranked, bm25_results, self.rrf_k, top_k);

        // Build a lookup map from vector results for content/metadata
        let vector_map: HashMap<String, &crate::qdrant::VectorSearchResult> = vector_results
            .iter()
            .map(|r| (r.point_id.clone(), r))
            .collect();

        // Assemble hybrid results
        let results = fused
            .into_iter()
            .map(|(chunk_id, score)| {
                let vr = vector_map.get(&chunk_id);
                HybridSearchResult {
                    chunk_id,
                    score,
                    content: vr.map(|r| r.content.clone()),
                    metadata: vr.map(|r| r.metadata.clone()),
                }
            })
            .collect();

        Ok(results)
    }

    /// Index a chunk into the vector store for later search.
    pub async fn index_chunk(
        &self,
        chunk_id: &str,
        content: &str,
        metadata: serde_json::Value,
    ) -> Result<()> {
        self.qdrant.upsert_text(chunk_id, content, metadata).await
    }

    /// Delete a chunk from the vector store.
    pub async fn delete_chunk(&self, chunk_id: &str) -> Result<bool> {
        self.qdrant.delete(chunk_id).await
    }

    /// Get a reference to the underlying QdrantStore.
    pub fn qdrant(&self) -> &QdrantStore {
        &self.qdrant
    }

    /// Run hybrid search with optional reranking.
    ///
    /// 1. Vector search + BM25 → RRF fusion (first-pass retrieval)
    /// 2. Reranker refines the top results (second-pass precision boost)
    pub async fn hybrid_search_with_rerank(
        &self,
        query: &str,
        bm25_results: Vec<(String, f64)>,
        reranker: &dyn crate::reranker::RerankerService,
        top_k: usize,
    ) -> Result<Vec<HybridSearchResult>> {
        // First pass: RRF fusion with over-fetch for reranker
        let rerank_candidates = top_k * 3;
        let fused = self.hybrid_search(query, bm25_results, rerank_candidates).await?;

        if fused.is_empty() {
            return Ok(fused);
        }

        // Collect texts for reranking
        let texts: Vec<String> = fused
            .iter()
            .map(|r| r.content.clone().unwrap_or_else(|| r.chunk_id.clone()))
            .collect();

        // Second pass: rerank
        let reranked = reranker.rerank(query, &texts, top_k).await?;

        // Map reranked results back to HybridSearchResult
        let results = reranked
            .into_iter()
            .filter_map(|rr| {
                fused.get(rr.index).map(|original| HybridSearchResult {
                    chunk_id: original.chunk_id.clone(),
                    score: rr.score as f64,
                    content: original.content.clone(),
                    metadata: original.metadata.clone(),
                })
            })
            .collect();

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rrf_fusion_basic() {
        // Two lists with overlapping chunk_ids
        let a = vec![
            ("c1".to_string(), 10.0),
            ("c2".to_string(), 8.0),
            ("c3".to_string(), 5.0),
        ];
        let b = vec![
            ("c2".to_string(), 12.0),
            ("c1".to_string(), 7.0),
            ("c4".to_string(), 3.0),
        ];

        let fused = rrf_fusion(a, b, 60.0, 10);

        // c1 appears at rank 1 in A and rank 2 in B => 1/61 + 1/62
        // c2 appears at rank 2 in A and rank 1 in B => 1/62 + 1/61
        // So c1 and c2 should have the same score
        assert!(fused.len() == 4);

        let c1_score = fused.iter().find(|(id, _)| id == "c1").unwrap().1;
        let c2_score = fused.iter().find(|(id, _)| id == "c2").unwrap().1;
        assert!((c1_score - c2_score).abs() < 1e-10, "c1 and c2 should have equal RRF scores");

        // c3 only in A at rank 3 => 1/63
        let c3_score = fused.iter().find(|(id, _)| id == "c3").unwrap().1;
        assert!((c3_score - 1.0 / 63.0).abs() < 1e-10);

        // c4 only in B at rank 3 => 1/63
        let c4_score = fused.iter().find(|(id, _)| id == "c4").unwrap().1;
        assert!((c4_score - 1.0 / 63.0).abs() < 1e-10);
    }

    #[test]
    fn test_rrf_fusion_disjoint() {
        let a = vec![("c1".to_string(), 5.0), ("c2".to_string(), 3.0)];
        let b = vec![("c3".to_string(), 9.0), ("c4".to_string(), 1.0)];

        let fused = rrf_fusion(a, b, 60.0, 10);
        assert_eq!(fused.len(), 4);

        // All items appear in exactly one list at either rank 1 or rank 2
        // rank 1 items: c1 (1/61) and c3 (1/61) — same score
        // rank 2 items: c2 (1/62) and c4 (1/62) — same score
        let c1 = fused.iter().find(|(id, _)| id == "c1").unwrap().1;
        let c3 = fused.iter().find(|(id, _)| id == "c3").unwrap().1;
        assert!((c1 - c3).abs() < 1e-10);

        let c2 = fused.iter().find(|(id, _)| id == "c2").unwrap().1;
        let c4 = fused.iter().find(|(id, _)| id == "c4").unwrap().1;
        assert!((c2 - c4).abs() < 1e-10);
        assert!(c1 > c2);
    }

    #[test]
    fn test_rrf_fusion_empty() {
        let fused = rrf_fusion(vec![], vec![], 60.0, 10);
        assert!(fused.is_empty());

        // One empty, one non-empty
        let a = vec![("c1".to_string(), 5.0)];
        let fused = rrf_fusion(a, vec![], 60.0, 10);
        assert_eq!(fused.len(), 1);
        assert_eq!(fused[0].0, "c1");
    }

    // -----------------------------------------------------------------------
    // HybridSearchEngine tests
    // -----------------------------------------------------------------------

    use crate::qdrant::{QdrantStore, StubEmbeddingService};

    fn make_engine() -> HybridSearchEngine {
        let store = QdrantStore::new("test", Box::new(StubEmbeddingService::new(64)));
        HybridSearchEngine::new(store)
    }

    /// Both vector and BM25 contribute results; RRF re-ranks them.
    #[tokio::test]
    async fn test_hybrid_search_combines_vector_and_bm25() {
        let engine = make_engine();

        // Index chunks into vector store
        engine.index_chunk("c1", "rust programming language systems", serde_json::json!({})).await.unwrap();
        engine.index_chunk("c2", "python scripting language dynamic", serde_json::json!({})).await.unwrap();
        engine.index_chunk("c3", "cooking recipes dinner ideas", serde_json::json!({})).await.unwrap();

        // BM25 results: c2 is ranked first, c1 second (different order from vector)
        let bm25 = vec![
            ("c2".to_string(), 5.0),
            ("c1".to_string(), 3.0),
            ("c3".to_string(), 1.0),
        ];

        let results = engine.hybrid_search("rust programming", bm25, 5).await.unwrap();

        // All 3 chunks should appear
        assert_eq!(results.len(), 3);

        // Every result should have an RRF score > 0
        for r in &results {
            assert!(r.score > 0.0, "chunk {} should have positive RRF score", r.chunk_id);
        }

        // c1 and c2 should both appear in top results (both present in both lists)
        let ids: Vec<&str> = results.iter().map(|r| r.chunk_id.as_str()).collect();
        assert!(ids.contains(&"c1"), "c1 should be in results");
        assert!(ids.contains(&"c2"), "c2 should be in results");

        // Verify that RRF fusion actually combines scores from both sources.
        // Each chunk appears in both the vector list and the BM25 list, so each
        // chunk's RRF score is the sum of two reciprocal rank terms:
        //   score_i = 1/(k + vector_rank_i) + 1/(k + bm25_rank_i)
        // The minimum possible single-source score for 3 items is 1/(60+3) = ~0.0159.
        // A dual-source score must exceed this because it's a sum of two terms.
        let min_dual_source = 1.0 / (60.0 + 3.0_f64); // worst single-source rank
        for r in &results {
            assert!(
                r.score > min_dual_source + 0.001,
                "chunk {} score {} should reflect contributions from both sources",
                r.chunk_id,
                r.score,
            );
        }

        // Verify content is populated from vector store
        let c1_result = results.iter().find(|r| r.chunk_id == "c1").unwrap();
        assert!(c1_result.content.is_some(), "content should be populated from vector store");
        assert!(c1_result.content.as_ref().unwrap().contains("rust"));
    }

    /// BM25 returns empty; only vector results contribute.
    #[tokio::test]
    async fn test_hybrid_search_vector_only() {
        let engine = make_engine();

        engine.index_chunk("c1", "rust systems programming", serde_json::json!({})).await.unwrap();
        engine.index_chunk("c2", "go concurrency model", serde_json::json!({})).await.unwrap();

        let bm25: Vec<(String, f64)> = vec![];

        let results = engine.hybrid_search("rust programming", bm25, 5).await.unwrap();

        assert!(!results.is_empty(), "should return vector results even without BM25");
        // All results come from vector search so all should have content
        for r in &results {
            assert!(r.content.is_some(), "vector-sourced results should have content");
            assert!(r.score > 0.0);
        }
    }

    /// Vector store is empty; only BM25 results contribute.
    #[tokio::test]
    async fn test_hybrid_search_bm25_only() {
        let engine = make_engine();
        // Don't index anything into vector store

        let bm25 = vec![
            ("c1".to_string(), 8.0),
            ("c2".to_string(), 4.0),
        ];

        let results = engine.hybrid_search("some query", bm25, 5).await.unwrap();

        assert_eq!(results.len(), 2, "should return BM25 results even without vector");
        assert_eq!(results[0].chunk_id, "c1", "c1 should be ranked first (higher BM25 rank)");
        assert_eq!(results[1].chunk_id, "c2");

        // Content should be None since these chunks are not in the vector store
        for r in &results {
            assert!(r.content.is_none(), "BM25-only results should not have content from vector store");
            assert!(r.score > 0.0);
        }
    }

    /// Both sources are empty.
    #[tokio::test]
    async fn test_hybrid_search_empty_both() {
        let engine = make_engine();

        let results = engine.hybrid_search("anything", vec![], 5).await.unwrap();
        assert!(results.is_empty(), "should return empty when both sources are empty");
    }

    /// Index 5 chunks, search, and verify the most relevant chunk is found.
    #[tokio::test]
    async fn test_index_and_search_round_trip() {
        let engine = make_engine();

        // Index 5 chunks with distinct content
        engine.index_chunk("c1", "rust ownership borrowing lifetimes", serde_json::json!({"topic": "rust"})).await.unwrap();
        engine.index_chunk("c2", "python decorators generators iterators", serde_json::json!({"topic": "python"})).await.unwrap();
        engine.index_chunk("c3", "javascript promises async await callbacks", serde_json::json!({"topic": "js"})).await.unwrap();
        engine.index_chunk("c4", "database indexing query optimization joins", serde_json::json!({"topic": "db"})).await.unwrap();
        engine.index_chunk("c5", "machine learning neural networks training", serde_json::json!({"topic": "ml"})).await.unwrap();

        // Verify all 5 are stored
        assert_eq!(engine.qdrant().count().await, 5);

        // Search for the exact text of c1 — with no BM25, it should be the top vector result
        let results = engine.hybrid_search("rust ownership borrowing lifetimes", vec![], 3).await.unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].chunk_id, "c1", "exact match should be ranked first");

        // Verify metadata is preserved
        let c1 = &results[0];
        assert!(c1.metadata.is_some());
        assert_eq!(c1.metadata.as_ref().unwrap()["topic"], "rust");

        // Delete a chunk and verify it's gone
        let removed = engine.delete_chunk("c3").await.unwrap();
        assert!(removed);
        assert_eq!(engine.qdrant().count().await, 4);

        // Searching for deleted content should not return c3
        let results = engine.hybrid_search("javascript promises async", vec![], 5).await.unwrap();
        let ids: Vec<&str> = results.iter().map(|r| r.chunk_id.as_str()).collect();
        assert!(!ids.contains(&"c3"), "deleted chunk should not appear in results");
    }
}
