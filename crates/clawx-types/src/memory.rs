use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, MemoryId};

/// Which scope a long-term memory belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryScope {
    /// Visible only to the owning Agent.
    Agent,
    /// Visible to all Agents (shared user knowledge).
    User,
}

/// Semantic kind of a memory entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryKind {
    Fact,
    Preference,
    Event,
    Skill,
    Contact,
    Terminology,
}

/// How a memory was created.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceType {
    /// Extracted by LLM from conversation.
    #[default]
    Implicit,
    /// User explicitly asked to remember.
    Explicit,
    /// Result of memory consolidation/merge.
    Consolidation,
}

/// A single long-term memory record.
/// Aligned with `memories` table in memory-architecture.md §4.2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: MemoryId,
    pub scope: MemoryScope,
    /// Agent ID — required when scope=Agent, optional for User.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub kind: MemoryKind,
    /// Short summary for display and quick matching.
    pub summary: String,
    /// JSON structured content.
    pub content: serde_json::Value,
    /// Importance score 0-10.
    pub importance: f64,
    /// Freshness 0-1 (Ebbinghaus decay).
    pub freshness: f64,
    pub access_count: u64,
    pub is_pinned: bool,
    /// Which Agent created this memory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_agent_id: Option<AgentId>,
    #[serde(default)]
    pub source_type: SourceType,
    /// If this memory was superseded by another.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<MemoryId>,
    /// Qdrant vector point ID (v0.2+, NULL in v0.1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub qdrant_point_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl MemoryEntry {
    /// Create a new MemoryEntry from a MemoryCandidate extracted by the LLM.
    pub fn from_candidate(candidate: MemoryCandidate, agent_id: Option<AgentId>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: MemoryId::new(),
            scope: candidate.scope,
            agent_id,
            kind: candidate.kind,
            summary: candidate.summary,
            content: candidate.content,
            importance: candidate.importance,
            freshness: 1.0,
            access_count: 0,
            is_pinned: false,
            source_agent_id: agent_id,
            source_type: candidate.source_type,
            superseded_by: None,
            qdrant_point_id: None,
            created_at: now,
            last_accessed_at: now,
            updated_at: now,
        }
    }
}

/// Query parameters for recalling memories.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query_text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default = "default_top_k")]
    pub top_k: usize,
    #[serde(default)]
    pub include_archived: bool,
    /// Max tokens budget for recalled memories.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_budget: Option<u32>,
}

fn default_top_k() -> usize {
    5
}

/// A memory entry with relevance scores.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredMemory {
    pub entry: MemoryEntry,
    pub semantic_score: f64,
    pub combined_score: f64,
}

/// Filter criteria for listing memories.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoryFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<MemoryScope>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MemoryKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keyword: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_importance: Option<f64>,
    #[serde(default)]
    pub include_archived: bool,
}

/// Payload for updating an existing memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryUpdate {
    pub id: MemoryId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub importance: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<MemoryKind>,
}

/// Statistics about memory usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryStats {
    pub total_count: u64,
    pub agent_count: u64,
    pub user_count: u64,
    pub pinned_count: u64,
    pub archived_count: u64,
}

/// A candidate memory extracted from conversation, before storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryCandidate {
    pub scope: MemoryScope,
    pub kind: MemoryKind,
    pub summary: String,
    pub content: serde_json::Value,
    pub importance: f64,
    pub source_type: SourceType,
}

/// Assembled context ready for LLM prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembledContext {
    pub system_prompt: String,
    pub recalled_memories: Vec<ScoredMemory>,
    pub knowledge_snippets: Vec<String>,
    pub conversation_history: Vec<crate::llm::Message>,
    pub total_tokens: u32,
    pub memory_tokens: u32,
}

/// Report from a decay run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecayReport {
    pub decayed_count: u64,
    pub archived_count: u64,
    pub deleted_count: u64,
}

/// Report from a consolidation run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConsolidationReport {
    pub merged_count: u64,
    pub superseded_count: u64,
}
