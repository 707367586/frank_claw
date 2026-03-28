//! Qdrant vector store integration for semantic search.
//!
//! Provides vector storage and retrieval using Qdrant's gRPC client.
//! Used alongside Tantivy BM25 for hybrid search (vector + keyword + RRF fusion).

use async_trait::async_trait;
use clawx_types::error::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Embedding service trait — abstracts the embedding model.
/// Implementations can use local models (nomic-embed-text, bge-m3)
/// or cloud APIs (OpenAI, Anthropic).
#[async_trait]
pub trait EmbeddingService: Send + Sync {
    /// Generate an embedding vector for the given text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Generate embeddings for a batch of texts.
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// The dimensionality of the embedding vectors.
    fn dimensions(&self) -> usize;
}

/// Stub embedding service for testing — produces deterministic vectors.
pub struct StubEmbeddingService {
    dims: usize,
}

impl StubEmbeddingService {
    pub fn new(dims: usize) -> Self {
        Self { dims }
    }
}

impl Default for StubEmbeddingService {
    fn default() -> Self {
        Self::new(768)
    }
}

#[async_trait]
impl EmbeddingService for StubEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        // Produce a deterministic vector based on text hash
        let hash = simple_hash(text);
        let mut vec = vec![0.0f32; self.dims];
        for (i, v) in vec.iter_mut().enumerate() {
            *v = ((hash.wrapping_add(i as u64) % 1000) as f32 / 1000.0) - 0.5;
        }
        // Normalize
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vec.iter_mut() {
                *v /= norm;
            }
        }
        Ok(vec)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.embed(text).await?);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.dims
    }
}

/// Simple hash function for deterministic test vectors.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for b in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(b as u64);
    }
    hash
}

/// A vector search result with score.
#[derive(Debug, Clone)]
pub struct VectorSearchResult {
    /// The ID of the matched point.
    pub point_id: String,
    /// Similarity score (higher = more similar).
    pub score: f32,
    /// The content stored with this point.
    pub content: String,
    /// Additional metadata.
    pub metadata: serde_json::Value,
}

/// A stored vector point with content and metadata.
#[derive(Debug, Clone)]
pub struct VectorPoint {
    /// Unique identifier for this point.
    pub point_id: String,
    /// The text content stored with this point.
    pub content: String,
    /// Additional metadata as JSON.
    pub metadata: serde_json::Value,
    /// The embedding vector.
    pub vector: Vec<f32>,
}

/// Qdrant vector store client wrapper.
/// Provides collection management and search operations.
/// Uses an in-memory store for local-first operation.
pub struct QdrantStore {
    collection_name: String,
    embedding: Box<dyn EmbeddingService>,
    points: Arc<RwLock<HashMap<String, VectorPoint>>>,
}

impl QdrantStore {
    /// Create a new Qdrant store with the given collection name and embedding service.
    pub fn new(collection_name: impl Into<String>, embedding: Box<dyn EmbeddingService>) -> Self {
        Self {
            collection_name: collection_name.into(),
            embedding,
            points: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Upsert a point with a pre-computed vector.
    pub async fn upsert(
        &self,
        point_id: impl Into<String>,
        content: impl Into<String>,
        metadata: serde_json::Value,
        vector: Vec<f32>,
    ) -> Result<()> {
        let id = point_id.into();
        let point = VectorPoint {
            point_id: id.clone(),
            content: content.into(),
            metadata,
            vector,
        };
        let mut map = self.points.write().await;
        map.insert(id, point);
        Ok(())
    }

    /// Upsert a point by auto-embedding the text content.
    pub async fn upsert_text(
        &self,
        point_id: impl Into<String>,
        content: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Result<()> {
        let content_str = content.into();
        let vector = self.embedding.embed(&content_str).await?;
        self.upsert(point_id, content_str, metadata, vector).await
    }

    /// Search by query text: embeds the query then finds top-k similar points.
    pub async fn search(&self, query_text: &str, top_k: usize) -> Result<Vec<VectorSearchResult>> {
        let query_vec = self.embedding.embed(query_text).await?;
        self.search_vector(&query_vec, top_k).await
    }

    /// Search by a pre-computed query vector, returning top-k results by cosine similarity.
    pub async fn search_vector(
        &self,
        query_vector: &[f32],
        top_k: usize,
    ) -> Result<Vec<VectorSearchResult>> {
        let map = self.points.read().await;
        let mut scored: Vec<VectorSearchResult> = map
            .values()
            .map(|pt| {
                let score = Self::cosine_similarity(query_vector, &pt.vector);
                VectorSearchResult {
                    point_id: pt.point_id.clone(),
                    score,
                    content: pt.content.clone(),
                    metadata: pt.metadata.clone(),
                }
            })
            .collect();
        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        Ok(scored)
    }

    /// Delete a point by ID. Returns true if the point existed.
    pub async fn delete(&self, point_id: &str) -> Result<bool> {
        let mut map = self.points.write().await;
        Ok(map.remove(point_id).is_some())
    }

    /// Return the number of stored points.
    pub async fn count(&self) -> usize {
        let map = self.points.read().await;
        map.len()
    }

    /// Get the collection name.
    pub fn collection_name(&self) -> &str {
        &self.collection_name
    }

    /// Get the embedding dimensionality.
    pub fn dimensions(&self) -> usize {
        self.embedding.dimensions()
    }

    /// Generate an embedding for text.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.embedding.embed(text).await
    }

    /// Generate embeddings for a batch of texts.
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        self.embedding.embed_batch(texts).await
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }
        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm_a == 0.0 || norm_b == 0.0 {
            return 0.0;
        }
        dot / (norm_a * norm_b)
    }
}

/// Reciprocal Rank Fusion (RRF) for combining multiple ranked lists.
/// Used to merge BM25 keyword results with vector similarity results.
pub fn reciprocal_rank_fusion(
    ranked_lists: &[Vec<(String, f32)>],
    k: f32,
) -> Vec<(String, f32)> {
    use std::collections::HashMap;

    let mut scores: HashMap<String, f32> = HashMap::new();

    for list in ranked_lists {
        for (rank, (id, _score)) in list.iter().enumerate() {
            let rrf_score = 1.0 / (k + rank as f32 + 1.0);
            *scores.entry(id.clone()).or_default() += rrf_score;
        }
    }

    let mut results: Vec<(String, f32)> = scores.into_iter().collect();
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // StubEmbeddingService
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn stub_embed_returns_correct_dimensions() {
        let service = StubEmbeddingService::new(128);
        let vec = service.embed("test text").await.unwrap();
        assert_eq!(vec.len(), 128);
    }

    #[tokio::test]
    async fn stub_embed_is_normalized() {
        let service = StubEmbeddingService::default();
        let vec = service.embed("hello world").await.unwrap();
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "vector should be normalized, got norm={}", norm);
    }

    #[tokio::test]
    async fn stub_embed_is_deterministic() {
        let service = StubEmbeddingService::default();
        let v1 = service.embed("same text").await.unwrap();
        let v2 = service.embed("same text").await.unwrap();
        assert_eq!(v1, v2);
    }

    #[tokio::test]
    async fn stub_embed_different_texts_differ() {
        let service = StubEmbeddingService::default();
        let v1 = service.embed("text one").await.unwrap();
        let v2 = service.embed("text two").await.unwrap();
        assert_ne!(v1, v2);
    }

    #[tokio::test]
    async fn stub_embed_batch() {
        let service = StubEmbeddingService::new(64);
        let texts = vec!["hello".to_string(), "world".to_string()];
        let results = service.embed_batch(&texts).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 64);
        assert_eq!(results[1].len(), 64);
    }

    // -----------------------------------------------------------------------
    // Cosine similarity
    // -----------------------------------------------------------------------

    #[test]
    fn cosine_similarity_identical_vectors() {
        let v = vec![1.0, 0.0, 0.0];
        let sim = QdrantStore::cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_orthogonal_vectors() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let sim = QdrantStore::cosine_similarity(&a, &b);
        assert!(sim.abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_opposite_vectors() {
        let a = vec![1.0, 0.0];
        let b = vec![-1.0, 0.0];
        let sim = QdrantStore::cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 0.001);
    }

    #[test]
    fn cosine_similarity_empty_vectors() {
        let a: Vec<f32> = vec![];
        let b: Vec<f32> = vec![];
        assert_eq!(QdrantStore::cosine_similarity(&a, &b), 0.0);
    }

    // -----------------------------------------------------------------------
    // QdrantStore
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn qdrant_store_embed() {
        let store = QdrantStore::new(
            "knowledge",
            Box::new(StubEmbeddingService::new(768)),
        );
        assert_eq!(store.collection_name(), "knowledge");
        assert_eq!(store.dimensions(), 768);

        let vec = store.embed("test query").await.unwrap();
        assert_eq!(vec.len(), 768);
    }

    #[tokio::test]
    async fn qdrant_store_embed_batch() {
        let store = QdrantStore::new(
            "memories",
            Box::new(StubEmbeddingService::new(768)),
        );
        let texts = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let vecs = store.embed_batch(&texts).await.unwrap();
        assert_eq!(vecs.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Reciprocal Rank Fusion
    // -----------------------------------------------------------------------

    #[test]
    fn rrf_single_list() {
        let lists = vec![vec![
            ("doc1".to_string(), 0.9),
            ("doc2".to_string(), 0.7),
            ("doc3".to_string(), 0.5),
        ]];
        let fused = reciprocal_rank_fusion(&lists, 60.0);
        assert_eq!(fused[0].0, "doc1");
        assert_eq!(fused.len(), 3);
    }

    #[test]
    fn rrf_two_lists_agree() {
        let lists = vec![
            vec![("doc1".to_string(), 0.9), ("doc2".to_string(), 0.7)],
            vec![("doc1".to_string(), 0.8), ("doc2".to_string(), 0.6)],
        ];
        let fused = reciprocal_rank_fusion(&lists, 60.0);
        // doc1 should be ranked first since it's #1 in both lists
        assert_eq!(fused[0].0, "doc1");
        assert!(fused[0].1 > fused[1].1);
    }

    #[test]
    fn rrf_two_lists_disagree() {
        let lists = vec![
            vec![("doc1".to_string(), 0.9), ("doc2".to_string(), 0.7)],
            vec![("doc2".to_string(), 0.8), ("doc1".to_string(), 0.6)],
        ];
        let fused = reciprocal_rank_fusion(&lists, 60.0);
        // Both docs have the same RRF score: 1/61 + 1/62
        assert_eq!(fused.len(), 2);
        // Scores should be equal since they're at position 1 and 2 in opposite lists
        assert!((fused[0].1 - fused[1].1).abs() < 0.001);
    }

    #[test]
    fn rrf_unique_docs_across_lists() {
        let lists = vec![
            vec![("doc1".to_string(), 0.9)],
            vec![("doc2".to_string(), 0.8)],
        ];
        let fused = reciprocal_rank_fusion(&lists, 60.0);
        assert_eq!(fused.len(), 2);
        // Both have equal RRF score since they're both rank 1 in their list
        assert!((fused[0].1 - fused[1].1).abs() < 0.001);
    }

    #[test]
    fn rrf_empty_lists() {
        let lists: Vec<Vec<(String, f32)>> = vec![];
        let fused = reciprocal_rank_fusion(&lists, 60.0);
        assert!(fused.is_empty());
    }

    // -----------------------------------------------------------------------
    // In-memory vector store: upsert / search / delete / count
    // -----------------------------------------------------------------------

    fn make_store() -> QdrantStore {
        QdrantStore::new("test", Box::new(StubEmbeddingService::new(64)))
    }

    #[tokio::test]
    async fn upsert_and_search_basic() {
        let store = make_store();
        // Insert 3 documents with distinct content
        store.upsert_text("doc1", "rust programming language", serde_json::json!({})).await.unwrap();
        store.upsert_text("doc2", "python programming language", serde_json::json!({})).await.unwrap();
        store.upsert_text("doc3", "cooking recipes for dinner", serde_json::json!({})).await.unwrap();

        // Search for something close to doc1
        let results = store.search("rust programming language", 3).await.unwrap();
        assert!(!results.is_empty());
        // The most similar result should be doc1 (identical text)
        assert_eq!(results[0].point_id, "doc1");
        assert!((results[0].score - 1.0).abs() < 0.001, "identical text should have score ~1.0");
    }

    #[tokio::test]
    async fn upsert_text_auto_embeds() {
        let store = make_store();
        store.upsert_text("doc1", "hello world", serde_json::json!({"source": "test"})).await.unwrap();

        // Verify the point was stored with a vector of the correct dimensions
        let map = store.points.read().await;
        let point = map.get("doc1").expect("doc1 should exist");
        assert_eq!(point.vector.len(), 64);
        assert_eq!(point.content, "hello world");
        assert_eq!(point.metadata, serde_json::json!({"source": "test"}));
    }

    #[tokio::test]
    async fn search_returns_top_k() {
        let store = make_store();
        for i in 0..10 {
            store
                .upsert_text(
                    &format!("doc{}", i),
                    &format!("document number {}", i),
                    serde_json::json!({}),
                )
                .await
                .unwrap();
        }
        let results = store.search("document number 5", 3).await.unwrap();
        assert_eq!(results.len(), 3, "should return exactly top_k results");
    }

    #[tokio::test]
    async fn search_empty_store_returns_empty() {
        let store = make_store();
        let results = store.search("anything", 5).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn delete_removes_point() {
        let store = make_store();
        store.upsert_text("doc1", "content", serde_json::json!({})).await.unwrap();
        assert_eq!(store.count().await, 1);

        let removed = store.delete("doc1").await.unwrap();
        assert!(removed, "delete should return true for existing point");
        assert_eq!(store.count().await, 0);

        let removed_again = store.delete("doc1").await.unwrap();
        assert!(!removed_again, "delete should return false for missing point");
    }

    #[tokio::test]
    async fn upsert_overwrites_existing() {
        let store = make_store();
        store.upsert_text("doc1", "original content", serde_json::json!({"v": 1})).await.unwrap();
        store.upsert_text("doc1", "updated content", serde_json::json!({"v": 2})).await.unwrap();

        assert_eq!(store.count().await, 1, "upsert should overwrite, not duplicate");

        let map = store.points.read().await;
        let point = map.get("doc1").unwrap();
        assert_eq!(point.content, "updated content");
        assert_eq!(point.metadata, serde_json::json!({"v": 2}));
    }

    #[tokio::test]
    async fn count_tracks_insertions() {
        let store = make_store();
        assert_eq!(store.count().await, 0);

        store.upsert_text("a", "aaa", serde_json::json!({})).await.unwrap();
        assert_eq!(store.count().await, 1);

        store.upsert_text("b", "bbb", serde_json::json!({})).await.unwrap();
        assert_eq!(store.count().await, 2);

        store.delete("a").await.unwrap();
        assert_eq!(store.count().await, 1);
    }

    #[tokio::test]
    async fn search_scores_ordered_descending() {
        let store = make_store();
        store.upsert_text("doc1", "alpha beta gamma", serde_json::json!({})).await.unwrap();
        store.upsert_text("doc2", "delta epsilon zeta", serde_json::json!({})).await.unwrap();
        store.upsert_text("doc3", "eta theta iota", serde_json::json!({})).await.unwrap();

        let results = store.search("alpha beta gamma", 3).await.unwrap();
        assert_eq!(results.len(), 3);
        // Scores must be in descending order
        for window in results.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "scores should be descending: {} >= {}",
                window[0].score,
                window[1].score,
            );
        }
        // Top result should be exact match
        assert_eq!(results[0].point_id, "doc1");
    }
}
