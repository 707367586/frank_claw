use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, ChunkId, DocumentId, KnowledgeSourceId};

/// A monitored knowledge source folder.
/// Aligned with `knowledge_sources` table in data-model.md §2.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeSource {
    pub id: KnowledgeSourceId,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub status: KnowledgeSourceStatus,
    pub file_count: u64,
    pub chunk_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KnowledgeSourceStatus {
    Active,
    Paused,
    Error,
}

/// An indexed document within a knowledge source.
/// Aligned with `documents` table in data-model.md §2.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub source_id: KnowledgeSourceId,
    pub file_path: String,
    pub file_type: String,
    pub file_hash: String,
    pub file_size: u64,
    pub chunk_count: u64,
    pub status: DocumentStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub indexed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentStatus {
    Pending,
    Indexed,
    Error,
}

/// A chunk of a document for vector indexing.
/// Aligned with `chunks` table in data-model.md §2.3.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    pub id: ChunkId,
    pub document_id: DocumentId,
    pub chunk_index: u32,
    pub content: String,
    pub token_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qdrant_point_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A search query for the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchQuery {
    pub query_text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default = "default_top_n")]
    pub top_n: usize,
}

fn default_top_n() -> usize {
    10
}

/// A single search result from the knowledge base.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub chunk: Chunk,
    pub document_path: String,
    pub score: f64,
}
