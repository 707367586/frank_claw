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

pub use stub::StubKnowledgeService;
pub use sqlite_kb::SqliteKnowledgeService;
pub use tantivy_index::TantivyIndex;
