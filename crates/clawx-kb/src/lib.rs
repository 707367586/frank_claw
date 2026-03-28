//! Knowledge engine for ClawX.
//!
//! Provides RAG-based knowledge retrieval, document ingestion,
//! and semantic search over agent knowledge bases.

mod stub;
mod parser;
mod chunker;
mod sqlite_kb;
pub mod tantivy_index;
pub mod hybrid;
pub mod qdrant;
pub mod embedding;
pub mod local_embedding;
pub mod reranker;

pub use stub::StubKnowledgeService;
pub use sqlite_kb::SqliteKnowledgeService;
pub use tantivy_index::TantivyIndex;
pub use qdrant::{EmbeddingService, QdrantStore, StubEmbeddingService, VectorPoint, reciprocal_rank_fusion};
pub use hybrid::{HybridSearchEngine, HybridSearchResult};
pub use embedding::{HttpEmbeddingService, EmbeddingConfig};
pub use local_embedding::{LocalEmbeddingService, LocalEmbeddingConfig};
pub use reranker::{RerankerService, RerankerConfig, RerankResult, HttpRerankerService, StubRerankerService};
