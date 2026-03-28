//! Reranker service for re-scoring retrieved documents against a query.
//!
//! Connects to a TEI-compatible `/rerank` endpoint (e.g. Qwen3-VL-Reranker-2B
//! served via HuggingFace Text Embeddings Inference).

use async_trait::async_trait;
use clawx_types::error::Result;
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single rerank result: original index, relevance score, and document text.
#[derive(Debug, Clone)]
pub struct RerankResult {
    /// The index of this document in the original input slice.
    pub index: usize,
    /// Relevance score assigned by the reranker (higher = more relevant).
    pub score: f32,
    /// The document text.
    pub text: String,
}

/// Configuration for the HTTP reranker service.
#[derive(Debug, Clone)]
pub struct RerankerConfig {
    /// Base URL of the TEI rerank endpoint.
    pub base_url: String,
    /// Model name (informational, not sent in the request).
    pub model_name: String,
}

impl Default for RerankerConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8081".to_string(),
            model_name: "Qwen/Qwen3-VL-Reranker-2B".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Trait
// ---------------------------------------------------------------------------

/// Reranker service trait — re-scores documents against a query.
#[async_trait]
pub trait RerankerService: Send + Sync {
    /// Rerank documents against a query.
    /// Returns `RerankResult` entries sorted by score descending, truncated to `top_k`.
    async fn rerank(
        &self,
        query: &str,
        documents: &[String],
        top_k: usize,
    ) -> Result<Vec<RerankResult>>;
}

// ---------------------------------------------------------------------------
// TEI wire types (serde)
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct TeiRerankRequest<'a> {
    query: &'a str,
    texts: &'a [String],
    truncate: bool,
}

#[derive(Deserialize)]
struct TeiRerankResponseItem {
    index: usize,
    score: f32,
}

// ---------------------------------------------------------------------------
// HttpRerankerService
// ---------------------------------------------------------------------------

/// Reranker that calls a TEI-compatible HTTP `/rerank` endpoint.
pub struct HttpRerankerService {
    client: reqwest::Client,
    config: RerankerConfig,
}

impl HttpRerankerService {
    /// Create a new `HttpRerankerService` with the given config.
    pub fn new(config: RerankerConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            config,
        }
    }

    /// Create with default config (localhost:8081).
    pub fn with_defaults() -> Self {
        Self::new(RerankerConfig::default())
    }
}

#[async_trait]
impl RerankerService for HttpRerankerService {
    async fn rerank(
        &self,
        query: &str,
        documents: &[String],
        top_k: usize,
    ) -> Result<Vec<RerankResult>> {
        if documents.is_empty() {
            return Ok(vec![]);
        }

        let url = format!("{}/rerank", self.config.base_url);
        let body = TeiRerankRequest {
            query,
            texts: documents,
            truncate: true,
        };

        let response = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| clawx_types::error::ClawxError::Internal(format!("reranker request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(clawx_types::error::ClawxError::Internal(
                format!("reranker returned HTTP {status}: {body_text}"),
            ));
        }

        let items: Vec<TeiRerankResponseItem> = response
            .json()
            .await
            .map_err(|e| clawx_types::error::ClawxError::Internal(format!("reranker response parse failed: {e}")))?;

        let mut results: Vec<RerankResult> = items
            .into_iter()
            .filter(|item| item.index < documents.len())
            .map(|item| RerankResult {
                index: item.index,
                score: item.score,
                text: documents[item.index].clone(),
            })
            .collect();

        // Sort by score descending
        results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);

        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// StubRerankerService
// ---------------------------------------------------------------------------

/// Stub reranker for testing — returns documents in original order
/// with linearly decreasing scores (1.0, 0.9, 0.8, ...).
pub struct StubRerankerService;

impl StubRerankerService {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StubRerankerService {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RerankerService for StubRerankerService {
    async fn rerank(
        &self,
        _query: &str,
        documents: &[String],
        top_k: usize,
    ) -> Result<Vec<RerankResult>> {
        let total = documents.len();
        let mut results: Vec<RerankResult> = documents
            .iter()
            .enumerate()
            .map(|(i, text)| {
                let score = if total <= 1 {
                    1.0
                } else {
                    1.0 - (i as f32 / total as f32)
                };
                RerankResult {
                    index: i,
                    score,
                    text: text.clone(),
                }
            })
            .collect();

        results.truncate(top_k);
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reranker_config_defaults() {
        let config = RerankerConfig::default();
        assert_eq!(config.base_url, "http://localhost:8081");
        assert_eq!(config.model_name, "Qwen/Qwen3-VL-Reranker-2B");
    }

    #[tokio::test]
    async fn test_stub_reranker_returns_all() {
        let reranker = StubRerankerService::new();
        let docs = vec![
            "first document".to_string(),
            "second document".to_string(),
            "third document".to_string(),
        ];

        let results = reranker.rerank("query", &docs, 10).await.unwrap();

        assert_eq!(results.len(), 3);
        // Original order preserved
        assert_eq!(results[0].index, 0);
        assert_eq!(results[1].index, 1);
        assert_eq!(results[2].index, 2);
        // Texts match
        assert_eq!(results[0].text, "first document");
        assert_eq!(results[1].text, "second document");
        assert_eq!(results[2].text, "third document");
        // Scores are decreasing
        assert!(results[0].score > results[1].score);
        assert!(results[1].score > results[2].score);
    }

    #[tokio::test]
    async fn test_stub_reranker_top_k() {
        let reranker = StubRerankerService::new();
        let docs = vec![
            "doc a".to_string(),
            "doc b".to_string(),
            "doc c".to_string(),
            "doc d".to_string(),
            "doc e".to_string(),
        ];

        let results = reranker.rerank("query", &docs, 2).await.unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].index, 0);
        assert_eq!(results[1].index, 1);
    }

    #[tokio::test]
    async fn test_stub_reranker_empty_docs() {
        let reranker = StubRerankerService::new();
        let docs: Vec<String> = vec![];

        let results = reranker.rerank("query", &docs, 5).await.unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_rerank_result_ordering() {
        let reranker = StubRerankerService::new();
        let docs: Vec<String> = (0..10)
            .map(|i| format!("document number {}", i))
            .collect();

        let results = reranker.rerank("query", &docs, 10).await.unwrap();

        // Verify scores are in descending order
        for window in results.windows(2) {
            assert!(
                window[0].score >= window[1].score,
                "scores should be descending: {} >= {}",
                window[0].score,
                window[1].score,
            );
        }
    }

    #[tokio::test]
    async fn test_stub_reranker_single_doc() {
        let reranker = StubRerankerService::new();
        let docs = vec!["only document".to_string()];

        let results = reranker.rerank("query", &docs, 5).await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].score, 1.0);
        assert_eq!(results[0].index, 0);
    }

    #[test]
    fn test_http_reranker_service_creates() {
        let config = RerankerConfig {
            base_url: "http://example.com:9090".to_string(),
            model_name: "test-model".to_string(),
        };
        let _service = HttpRerankerService::new(config);
        // Verify it can be constructed without panicking
    }

    #[test]
    fn test_http_reranker_with_defaults() {
        let _service = HttpRerankerService::with_defaults();
        // Verify default construction works
    }
}
