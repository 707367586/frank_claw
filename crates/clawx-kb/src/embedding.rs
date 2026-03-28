//! HTTP-based embedding service for OpenAI-compatible `/v1/embeddings` endpoints.
//!
//! Works with HuggingFace TEI, vLLM, Ollama, or any OpenAI-compatible embedding API.
//! Default configuration targets Qwen3-VL-Embedding-2B served via TEI.

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use serde::{Deserialize, Serialize};

use crate::qdrant::EmbeddingService;

/// Configuration for the HTTP embedding service.
#[derive(Debug, Clone)]
pub struct EmbeddingConfig {
    /// Base URL of the embedding API (e.g., `http://localhost:8080`).
    pub base_url: String,
    /// Model name sent in the request body.
    pub model_name: String,
    /// Optional Bearer token for authenticated endpoints.
    pub api_key: Option<String>,
    /// Dimensionality of the embedding vectors.
    pub dimensions: usize,
    /// Maximum number of texts per API call.
    pub batch_size: usize,
}

impl Default for EmbeddingConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:8080".to_string(),
            model_name: "Qwen/Qwen3-VL-Embedding-2B".to_string(),
            api_key: None,
            dimensions: 1536,
            batch_size: 32,
        }
    }
}

/// Request body for the OpenAI-compatible embeddings API.
#[derive(Debug, Serialize)]
struct EmbeddingRequest {
    model: String,
    input: Vec<String>,
}

/// A single embedding entry in the API response.
#[derive(Debug, Deserialize)]
struct EmbeddingData {
    embedding: Vec<f32>,
    #[allow(dead_code)]
    index: usize,
}

/// Response body from the OpenAI-compatible embeddings API.
#[derive(Debug, Deserialize)]
struct EmbeddingResponse {
    data: Vec<EmbeddingData>,
}

/// HTTP-based embedding service that connects to an OpenAI-compatible
/// `/v1/embeddings` endpoint.
pub struct HttpEmbeddingService {
    config: EmbeddingConfig,
    client: reqwest::Client,
}

impl HttpEmbeddingService {
    /// Create a new HTTP embedding service with the given configuration.
    pub fn new(config: EmbeddingConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    /// Build and send an embedding request for the given texts.
    async fn call_api(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let url = format!("{}/v1/embeddings", self.config.base_url);

        let body = EmbeddingRequest {
            model: self.config.model_name.clone(),
            input: texts,
        };

        let mut request = self.client.post(&url).json(&body);

        if let Some(ref key) = self.config.api_key {
            request = request.header("Authorization", format!("Bearer {}", key));
        }

        let response = request.send().await.map_err(|e| {
            ClawxError::VectorStore(format!("embedding API request failed: {}", e))
        })?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(ClawxError::VectorStore(format!(
                "embedding API returned {}: {}",
                status, body_text
            )));
        }

        let resp: EmbeddingResponse = response.json().await.map_err(|e| {
            ClawxError::VectorStore(format!("failed to parse embedding response: {}", e))
        })?;

        // Sort by index to ensure correct ordering
        let mut data = resp.data;
        data.sort_by_key(|d| d.index);

        Ok(data.into_iter().map(|d| d.embedding).collect())
    }
}

#[async_trait]
impl EmbeddingService for HttpEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.call_api(vec![text.to_string()]).await?;
        results
            .into_iter()
            .next()
            .ok_or_else(|| ClawxError::VectorStore("embedding API returned empty data".into()))
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let mut all_embeddings = Vec::with_capacity(texts.len());

        for chunk in texts.chunks(self.config.batch_size) {
            let batch_result = self.call_api(chunk.to_vec()).await?;
            all_embeddings.extend(batch_result);
        }

        Ok(all_embeddings)
    }

    fn dimensions(&self) -> usize {
        self.config.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // EmbeddingConfig defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_embedding_config_defaults() {
        let config = EmbeddingConfig::default();
        assert_eq!(config.base_url, "http://localhost:8080");
        assert_eq!(config.model_name, "Qwen/Qwen3-VL-Embedding-2B");
        assert!(config.api_key.is_none());
        assert_eq!(config.dimensions, 1536);
        assert_eq!(config.batch_size, 32);
    }

    #[test]
    fn test_embedding_config_custom() {
        let config = EmbeddingConfig {
            base_url: "https://api.example.com".to_string(),
            model_name: "text-embedding-3-small".to_string(),
            api_key: Some("sk-test-key".to_string()),
            dimensions: 768,
            batch_size: 64,
        };
        assert_eq!(config.base_url, "https://api.example.com");
        assert_eq!(config.model_name, "text-embedding-3-small");
        assert_eq!(config.api_key.as_deref(), Some("sk-test-key"));
        assert_eq!(config.dimensions, 768);
        assert_eq!(config.batch_size, 64);
    }

    // -----------------------------------------------------------------------
    // Request format
    // -----------------------------------------------------------------------

    #[test]
    fn test_embedding_request_format() {
        let req = EmbeddingRequest {
            model: "Qwen/Qwen3-VL-Embedding-2B".to_string(),
            input: vec!["hello world".to_string(), "foo bar".to_string()],
        };
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["model"], "Qwen/Qwen3-VL-Embedding-2B");
        assert!(json["input"].is_array());
        let input_arr = json["input"].as_array().unwrap();
        assert_eq!(input_arr.len(), 2);
        assert_eq!(input_arr[0], "hello world");
        assert_eq!(input_arr[1], "foo bar");
    }

    #[test]
    fn test_embedding_request_single_input() {
        let req = EmbeddingRequest {
            model: "test-model".to_string(),
            input: vec!["single text".to_string()],
        };
        let json = serde_json::to_value(&req).unwrap();

        assert_eq!(json["model"], "test-model");
        assert_eq!(json["input"].as_array().unwrap().len(), 1);
    }

    // -----------------------------------------------------------------------
    // Response deserialization
    // -----------------------------------------------------------------------

    #[test]
    fn test_embedding_response_deserialization() {
        let json_str = r#"{
            "data": [
                {"embedding": [0.1, 0.2, 0.3], "index": 0},
                {"embedding": [0.4, 0.5, 0.6], "index": 1}
            ]
        }"#;
        let resp: EmbeddingResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].embedding, vec![0.1, 0.2, 0.3]);
        assert_eq!(resp.data[0].index, 0);
        assert_eq!(resp.data[1].embedding, vec![0.4, 0.5, 0.6]);
        assert_eq!(resp.data[1].index, 1);
    }

    #[test]
    fn test_embedding_response_out_of_order() {
        let json_str = r#"{
            "data": [
                {"embedding": [0.4, 0.5, 0.6], "index": 1},
                {"embedding": [0.1, 0.2, 0.3], "index": 0}
            ]
        }"#;
        let resp: EmbeddingResponse = serde_json::from_str(json_str).unwrap();
        assert_eq!(resp.data.len(), 2);
        // index 1 comes first in the raw response
        assert_eq!(resp.data[0].index, 1);
        assert_eq!(resp.data[1].index, 0);
    }

    // -----------------------------------------------------------------------
    // Dimensions
    // -----------------------------------------------------------------------

    #[test]
    fn test_embedding_dimensions_default() {
        let service = HttpEmbeddingService::new(EmbeddingConfig::default());
        assert_eq!(service.dimensions(), 1536);
    }

    #[test]
    fn test_embedding_dimensions_custom() {
        let config = EmbeddingConfig {
            dimensions: 384,
            ..EmbeddingConfig::default()
        };
        let service = HttpEmbeddingService::new(config);
        assert_eq!(service.dimensions(), 384);
    }

    // -----------------------------------------------------------------------
    // Service construction
    // -----------------------------------------------------------------------

    #[test]
    fn test_service_construction_with_api_key() {
        let config = EmbeddingConfig {
            api_key: Some("test-key-123".to_string()),
            ..EmbeddingConfig::default()
        };
        let service = HttpEmbeddingService::new(config);
        assert_eq!(service.config.api_key.as_deref(), Some("test-key-123"));
    }

    #[test]
    fn test_service_construction_without_api_key() {
        let service = HttpEmbeddingService::new(EmbeddingConfig::default());
        assert!(service.config.api_key.is_none());
    }

    // -----------------------------------------------------------------------
    // Embed batch with empty input
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_embed_batch_empty_input() {
        let service = HttpEmbeddingService::new(EmbeddingConfig::default());
        let result = service.embed_batch(&[]).await.unwrap();
        assert!(result.is_empty());
    }

    // -----------------------------------------------------------------------
    // Error handling: connection refused
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_embed_invalid_endpoint() {
        let config = EmbeddingConfig {
            base_url: "http://127.0.0.1:19999".to_string(),
            ..EmbeddingConfig::default()
        };
        let service = HttpEmbeddingService::new(config);
        let result = service.embed("test").await;
        assert!(result.is_err(), "should fail when endpoint is unavailable");
        let msg = format!("{}", result.unwrap_err());
        // Could be either a network error or an HTTP error depending on environment
        assert!(
            msg.contains("embedding API"),
            "expected embedding API error, got: {}",
            msg
        );
    }

    #[tokio::test]
    async fn test_embed_batch_invalid_endpoint() {
        let config = EmbeddingConfig {
            base_url: "http://127.0.0.1:19999".to_string(),
            ..EmbeddingConfig::default()
        };
        let service = HttpEmbeddingService::new(config);
        let texts = vec!["hello".to_string(), "world".to_string()];
        let result = service.embed_batch(&texts).await;
        assert!(result.is_err(), "should fail when endpoint is unavailable");
    }

    // -----------------------------------------------------------------------
    // Integration test (ignored by default, requires real API)
    // -----------------------------------------------------------------------

    #[tokio::test]
    #[ignore]
    async fn test_http_embedding_real_api() {
        let config = EmbeddingConfig {
            base_url: "http://localhost:8080".to_string(),
            model_name: "Qwen/Qwen3-VL-Embedding-2B".to_string(),
            api_key: None,
            dimensions: 1536,
            batch_size: 32,
        };
        let service = HttpEmbeddingService::new(config);

        // Single embed
        let vec = service.embed("Hello, world!").await.unwrap();
        assert!(!vec.is_empty(), "embedding should not be empty");
        assert_eq!(vec.len(), 1536, "embedding should have 1536 dimensions");

        // Batch embed
        let texts = vec![
            "Rust programming language".to_string(),
            "Python programming language".to_string(),
        ];
        let vecs = service.embed_batch(&texts).await.unwrap();
        assert_eq!(vecs.len(), 2);
        assert_eq!(vecs[0].len(), 1536);
        assert_eq!(vecs[1].len(), 1536);

        // Vectors for different texts should differ
        assert_ne!(vecs[0], vecs[1]);
    }
}
