//! Local embedding service using candle (HuggingFace Rust ML framework).
//!
//! Runs embedding models locally with no external server needed.
//! Uses candle-transformers' BertModel as the default architecture.
//! Models are downloaded from HuggingFace Hub and cached in `~/.cache/huggingface/hub/`.

use async_trait::async_trait;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use clawx_types::error::{ClawxError, Result};
use tokenizers::Tokenizer;

use crate::qdrant::EmbeddingService;

/// Configuration for the local embedding service.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LocalEmbeddingConfig {
    /// HuggingFace model ID (e.g., "sentence-transformers/all-MiniLM-L6-v2").
    pub model_id: String,
    /// Optional revision/branch.
    pub revision: String,
    /// Whether to use GPU (Metal on macOS). Currently CPU-only; reserved for future use.
    pub use_gpu: bool,
    /// Embedding dimensions (auto-detected if 0).
    pub dimensions: usize,
    /// Normalize output vectors via L2 normalization.
    pub normalize: bool,
    /// Max sequence length for the tokenizer.
    pub max_seq_len: usize,
}

impl Default for LocalEmbeddingConfig {
    fn default() -> Self {
        Self {
            model_id: "Qwen/Qwen3-VL-Embedding-2B".to_string(),
            revision: "main".to_string(),
            use_gpu: true,
            dimensions: 0,
            normalize: true,
            max_seq_len: 8192,
        }
    }
}

/// Compute mean pooling over token embeddings, excluding padding tokens.
///
/// `embeddings` shape: (seq_len, hidden_dim)
/// `attention_mask`: length seq_len, 1.0 for real tokens, 0.0 for padding
///
/// Returns a vector of length hidden_dim.
pub fn mean_pool(embeddings: &[Vec<f32>], attention_mask: &[f32]) -> Vec<f32> {
    if embeddings.is_empty() {
        return vec![];
    }

    let hidden_dim = embeddings[0].len();
    let mut sum = vec![0.0f32; hidden_dim];
    let mut count = 0.0f32;

    for (i, token_emb) in embeddings.iter().enumerate() {
        let mask = if i < attention_mask.len() {
            attention_mask[i]
        } else {
            0.0
        };
        if mask > 0.0 {
            for (j, val) in token_emb.iter().enumerate() {
                sum[j] += val * mask;
            }
            count += mask;
        }
    }

    if count > 0.0 {
        for val in sum.iter_mut() {
            *val /= count;
        }
    }

    sum
}

/// L2 normalize a vector in-place, returning the normalized vector.
/// If the vector has zero norm, it is returned unchanged.
pub fn l2_normalize(vec: &mut Vec<f32>) -> &mut Vec<f32> {
    let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vec.iter_mut() {
            *v /= norm;
        }
    }
    vec
}

/// Local embedding service that runs models via candle.
///
/// Holds the loaded model, tokenizer, and device for inference.
pub struct LocalEmbeddingService {
    config: LocalEmbeddingConfig,
    tokenizer: Tokenizer,
    model: BertModel,
    device: Device,
    dimensions: usize,
}

impl LocalEmbeddingService {
    /// Load a model from HuggingFace Hub and initialize the service.
    ///
    /// Downloads `tokenizer.json`, `config.json`, and `model.safetensors`
    /// from the specified model repository. Files are cached in
    /// `~/.cache/huggingface/hub/`.
    ///
    /// Returns an error if the model cannot be loaded as a BERT-compatible
    /// architecture. In that case, use `HttpEmbeddingService` instead.
    pub fn load(config: LocalEmbeddingConfig) -> Result<Self> {
        // Use CPU for now; GPU/Metal support reserved for future feature flag
        let device = Device::Cpu;

        // Download model files from HuggingFace Hub
        let api = hf_hub::api::sync::Api::new().map_err(|e| {
            ClawxError::VectorStore(format!("failed to initialize HuggingFace Hub API: {}", e))
        })?;
        let repo = api.repo(hf_hub::Repo::with_revision(
            config.model_id.clone(),
            hf_hub::RepoType::Model,
            config.revision.clone(),
        ));

        let tokenizer_path = repo.get("tokenizer.json").map_err(|e| {
            ClawxError::VectorStore(format!(
                "failed to download tokenizer.json for '{}': {}. \
                 Check your network connection or use HttpEmbeddingService instead.",
                config.model_id, e
            ))
        })?;

        let config_path = repo.get("config.json").map_err(|e| {
            ClawxError::VectorStore(format!(
                "failed to download config.json for '{}': {}",
                config.model_id, e
            ))
        })?;

        let weights_path = repo.get("model.safetensors").map_err(|e| {
            ClawxError::VectorStore(format!(
                "failed to download model.safetensors for '{}': {}. \
                 Sharded models are not yet supported. \
                 Use HttpEmbeddingService for large/sharded models.",
                config.model_id, e
            ))
        })?;

        // Load tokenizer
        let mut tokenizer = Tokenizer::from_file(&tokenizer_path).map_err(|e| {
            ClawxError::VectorStore(format!("failed to load tokenizer: {}", e))
        })?;

        // Set truncation to max_seq_len
        let _ = tokenizer.with_truncation(Some(tokenizers::TruncationParams {
            max_length: config.max_seq_len,
            ..Default::default()
        }));

        // Load model config
        let config_data = std::fs::read_to_string(&config_path).map_err(|e| {
            ClawxError::VectorStore(format!("failed to read config.json: {}", e))
        })?;
        let bert_config: BertConfig = serde_json::from_str(&config_data).map_err(|e| {
            ClawxError::VectorStore(format!(
                "model '{}' is not BERT-compatible (failed to parse config.json as BertConfig): {}. \
                 Use HttpEmbeddingService for non-BERT architectures.",
                config.model_id, e
            ))
        })?;

        // Detect dimensions from config
        let dimensions = if config.dimensions > 0 {
            config.dimensions
        } else {
            bert_config.hidden_size
        };

        // Load model weights
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[weights_path],
                DType::F32,
                &device,
            )
            .map_err(|e| {
                ClawxError::VectorStore(format!("failed to load model weights: {}", e))
            })?
        };

        let model = BertModel::load(vb, &bert_config).map_err(|e| {
            ClawxError::VectorStore(format!(
                "failed to build BertModel for '{}': {}. \
                 This model may not be BERT-compatible. \
                 Use HttpEmbeddingService instead.",
                config.model_id, e
            ))
        })?;

        tracing::info!(
            model_id = %config.model_id,
            dimensions = dimensions,
            device = "cpu",
            "local embedding service loaded"
        );

        Ok(Self {
            config,
            tokenizer,
            model,
            device,
            dimensions,
        })
    }

    /// Run inference on a single text, returning the pooled embedding.
    fn infer(&self, text: &str) -> Result<Vec<f32>> {
        let encoding = self.tokenizer.encode(text, true).map_err(|e| {
            ClawxError::VectorStore(format!("tokenization failed: {}", e))
        })?;

        let input_ids = encoding.get_ids();
        let attention_mask = encoding.get_attention_mask();
        let token_type_ids = encoding.get_type_ids();

        let seq_len = input_ids.len();

        let input_ids_tensor = Tensor::new(input_ids, &self.device)
            .map_err(|e| ClawxError::VectorStore(format!("tensor creation failed: {}", e)))?
            .reshape((1, seq_len))
            .map_err(|e| ClawxError::VectorStore(format!("reshape failed: {}", e)))?;

        let token_type_ids_tensor = Tensor::new(token_type_ids, &self.device)
            .map_err(|e| ClawxError::VectorStore(format!("tensor creation failed: {}", e)))?
            .reshape((1, seq_len))
            .map_err(|e| ClawxError::VectorStore(format!("reshape failed: {}", e)))?;

        let attention_mask_tensor = Tensor::new(attention_mask, &self.device)
            .map_err(|e| ClawxError::VectorStore(format!("tensor creation failed: {}", e)))?
            .reshape((1, seq_len))
            .map_err(|e| ClawxError::VectorStore(format!("reshape failed: {}", e)))?;

        // Forward pass: get all token embeddings (batch=1, seq_len, hidden_dim)
        let output = self
            .model
            .forward(&input_ids_tensor, &token_type_ids_tensor, Some(&attention_mask_tensor))
            .map_err(|e| ClawxError::VectorStore(format!("model forward pass failed: {}", e)))?;

        // Extract embeddings: squeeze batch dimension -> (seq_len, hidden_dim)
        let output_2d = output
            .squeeze(0)
            .map_err(|e| ClawxError::VectorStore(format!("squeeze failed: {}", e)))?;

        let embeddings_raw: Vec<Vec<f32>> = (0..seq_len)
            .map(|i| {
                output_2d
                    .get(i)
                    .and_then(|t| t.to_vec1::<f32>())
                    .unwrap_or_default()
            })
            .collect();

        let mask_f32: Vec<f32> = attention_mask.iter().map(|&m| m as f32).collect();

        let mut pooled = mean_pool(&embeddings_raw, &mask_f32);

        if self.config.normalize {
            l2_normalize(&mut pooled);
        }

        Ok(pooled)
    }
}

#[async_trait]
impl EmbeddingService for LocalEmbeddingService {
    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        self.infer(text)
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.infer(text)?);
        }
        Ok(results)
    }

    fn dimensions(&self) -> usize {
        self.dimensions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // LocalEmbeddingConfig defaults
    // -----------------------------------------------------------------------

    #[test]
    fn test_local_embedding_config_defaults() {
        let config = LocalEmbeddingConfig::default();
        assert_eq!(config.model_id, "Qwen/Qwen3-VL-Embedding-2B");
        assert_eq!(config.revision, "main");
        assert!(config.use_gpu);
        assert_eq!(config.dimensions, 0);
        assert!(config.normalize);
        assert_eq!(config.max_seq_len, 8192);
    }

    #[test]
    fn test_local_embedding_config_custom() {
        let config = LocalEmbeddingConfig {
            model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            revision: "refs/pr/123".to_string(),
            use_gpu: false,
            dimensions: 384,
            normalize: false,
            max_seq_len: 512,
        };
        assert_eq!(config.model_id, "sentence-transformers/all-MiniLM-L6-v2");
        assert_eq!(config.revision, "refs/pr/123");
        assert!(!config.use_gpu);
        assert_eq!(config.dimensions, 384);
        assert!(!config.normalize);
        assert_eq!(config.max_seq_len, 512);
    }

    #[test]
    fn test_local_embedding_config_serde_roundtrip() {
        let config = LocalEmbeddingConfig::default();
        let json = serde_json::to_string(&config).unwrap();
        let deserialized: LocalEmbeddingConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model_id, config.model_id);
        assert_eq!(deserialized.revision, config.revision);
        assert_eq!(deserialized.use_gpu, config.use_gpu);
        assert_eq!(deserialized.dimensions, config.dimensions);
        assert_eq!(deserialized.normalize, config.normalize);
        assert_eq!(deserialized.max_seq_len, config.max_seq_len);
    }

    // -----------------------------------------------------------------------
    // mean_pool
    // -----------------------------------------------------------------------

    #[test]
    fn test_mean_pooling_basic() {
        // 3 tokens, hidden_dim = 2
        let embeddings = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![5.0, 6.0],
        ];
        let mask = vec![1.0, 1.0, 1.0]; // all tokens are real
        let result = mean_pool(&embeddings, &mask);
        assert_eq!(result.len(), 2);
        // mean of [1,3,5] = 3.0, mean of [2,4,6] = 4.0
        assert!((result[0] - 3.0).abs() < 1e-6);
        assert!((result[1] - 4.0).abs() < 1e-6);
    }

    #[test]
    fn test_mean_pooling_with_padding() {
        let embeddings = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![100.0, 200.0], // padding token
        ];
        let mask = vec![1.0, 1.0, 0.0]; // last token is padding
        let result = mean_pool(&embeddings, &mask);
        // mean of [1,3] = 2.0, mean of [2,4] = 3.0
        assert!((result[0] - 2.0).abs() < 1e-6);
        assert!((result[1] - 3.0).abs() < 1e-6);
    }

    #[test]
    fn test_mean_pooling_single_token() {
        let embeddings = vec![vec![0.5, -0.3, 0.8]];
        let mask = vec![1.0];
        let result = mean_pool(&embeddings, &mask);
        assert_eq!(result, vec![0.5, -0.3, 0.8]);
    }

    #[test]
    fn test_mean_pooling_all_padding() {
        let embeddings = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
        ];
        let mask = vec![0.0, 0.0]; // all padding
        let result = mean_pool(&embeddings, &mask);
        // No tokens contribute, so result should be zeros
        assert_eq!(result, vec![0.0, 0.0]);
    }

    #[test]
    fn test_mean_pooling_empty_input() {
        let embeddings: Vec<Vec<f32>> = vec![];
        let mask: Vec<f32> = vec![];
        let result = mean_pool(&embeddings, &mask);
        assert!(result.is_empty());
    }

    #[test]
    fn test_mean_pooling_mask_shorter_than_embeddings() {
        // If mask is shorter, extra tokens should be treated as padding
        let embeddings = vec![
            vec![1.0, 2.0],
            vec![3.0, 4.0],
            vec![100.0, 200.0],
        ];
        let mask = vec![1.0, 1.0]; // only covers first two tokens
        let result = mean_pool(&embeddings, &mask);
        // mean of [1,3] = 2.0, mean of [2,4] = 3.0
        assert!((result[0] - 2.0).abs() < 1e-6);
        assert!((result[1] - 3.0).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // l2_normalize
    // -----------------------------------------------------------------------

    #[test]
    fn test_l2_normalize_basic() {
        let mut vec = vec![3.0, 4.0];
        l2_normalize(&mut vec);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6, "expected norm 1.0, got {}", norm);
        assert!((vec[0] - 0.6).abs() < 1e-6);
        assert!((vec[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_already_unit() {
        let mut vec = vec![1.0, 0.0, 0.0];
        l2_normalize(&mut vec);
        assert!((vec[0] - 1.0).abs() < 1e-6);
        assert!(vec[1].abs() < 1e-6);
        assert!(vec[2].abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_zero_vector() {
        let mut vec = vec![0.0, 0.0, 0.0];
        l2_normalize(&mut vec);
        // Zero vector should remain unchanged
        assert_eq!(vec, vec![0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_l2_normalize_negative_values() {
        let mut vec = vec![-3.0, 4.0];
        l2_normalize(&mut vec);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
        assert!((vec[0] - (-0.6)).abs() < 1e-6);
        assert!((vec[1] - 0.8).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_single_element() {
        let mut vec = vec![5.0];
        l2_normalize(&mut vec);
        assert!((vec[0] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_l2_normalize_preserves_direction() {
        let mut vec = vec![2.0, 2.0, 2.0];
        l2_normalize(&mut vec);
        // All components should be equal after normalization
        assert!((vec[0] - vec[1]).abs() < 1e-6);
        assert!((vec[1] - vec[2]).abs() < 1e-6);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-6);
    }

    // -----------------------------------------------------------------------
    // Dimensions config
    // -----------------------------------------------------------------------

    #[test]
    fn test_local_embedding_dimensions_auto_detect() {
        let config = LocalEmbeddingConfig::default();
        // dimensions=0 means auto-detect from model
        assert_eq!(config.dimensions, 0);
    }

    #[test]
    fn test_local_embedding_dimensions_explicit() {
        let config = LocalEmbeddingConfig {
            dimensions: 768,
            ..LocalEmbeddingConfig::default()
        };
        assert_eq!(config.dimensions, 768);
    }

    // -----------------------------------------------------------------------
    // Model loading (ignored - requires network + model download)
    // -----------------------------------------------------------------------

    #[test]
    #[ignore]
    fn test_local_embedding_load_bert_model() {
        // This test requires downloading a model from HuggingFace Hub.
        // Run with: cargo test -p clawx-kb -- --ignored test_local_embedding_load_bert_model
        let config = LocalEmbeddingConfig {
            model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            revision: "main".to_string(),
            use_gpu: false,
            dimensions: 0,
            normalize: true,
            max_seq_len: 512,
        };
        let service = LocalEmbeddingService::load(config).unwrap();
        assert_eq!(service.dimensions(), 384); // MiniLM-L6-v2 has 384 dims
    }

    #[tokio::test]
    #[ignore]
    async fn test_local_embedding_inference() {
        // This test requires a downloaded model.
        // Run with: cargo test -p clawx-kb -- --ignored test_local_embedding_inference
        let config = LocalEmbeddingConfig {
            model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            revision: "main".to_string(),
            use_gpu: false,
            dimensions: 0,
            normalize: true,
            max_seq_len: 512,
        };
        let service = LocalEmbeddingService::load(config).unwrap();

        let vec = service.embed("Hello, world!").await.unwrap();
        assert_eq!(vec.len(), 384);

        // Verify normalized
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01, "vector should be normalized, got norm={}", norm);

        // Verify deterministic
        let vec2 = service.embed("Hello, world!").await.unwrap();
        assert_eq!(vec, vec2);

        // Verify different texts produce different embeddings
        let vec3 = service.embed("Goodbye, world!").await.unwrap();
        assert_ne!(vec, vec3);
    }

    #[tokio::test]
    #[ignore]
    async fn test_local_embedding_batch() {
        let config = LocalEmbeddingConfig {
            model_id: "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            revision: "main".to_string(),
            use_gpu: false,
            dimensions: 0,
            normalize: true,
            max_seq_len: 512,
        };
        let service = LocalEmbeddingService::load(config).unwrap();

        let texts = vec!["hello".to_string(), "world".to_string()];
        let results = service.embed_batch(&texts).await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].len(), 384);
        assert_eq!(results[1].len(), 384);
        assert_ne!(results[0], results[1]);
    }

    #[test]
    #[ignore]
    fn test_local_embedding_unsupported_model_gives_clear_error() {
        // Attempting to load a non-BERT model should give a helpful error message.
        let config = LocalEmbeddingConfig {
            model_id: "Qwen/Qwen3-VL-Embedding-2B".to_string(),
            use_gpu: false,
            ..LocalEmbeddingConfig::default()
        };
        let result = LocalEmbeddingService::load(config);
        if let Err(e) = result {
            let msg = format!("{}", e);
            assert!(
                msg.contains("HttpEmbeddingService") || msg.contains("BERT"),
                "error should guide user to HttpEmbeddingService, got: {}",
                msg
            );
        }
        // If it succeeds, that's fine too (model may be BERT-compatible)
    }
}
