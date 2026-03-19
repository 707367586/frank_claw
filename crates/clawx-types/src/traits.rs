use async_trait::async_trait;
use std::pin::Pin;

use crate::agent::Conversation;
use crate::error::Result;
use crate::ids::*;
use crate::knowledge::{SearchQuery, SearchResult};
use crate::llm::{CompletionRequest, LlmResponse, LlmStreamChunk};
use crate::memory::*;
use crate::pagination::{PagedResult, Pagination};
use crate::security::{Capability, DlpResult, SecurityDecision};
use crate::vault::{DiffPreview, VaultSnapshot};

// ---------------------------------------------------------------------------
// Memory traits (memory-architecture.md §5.1)
// ---------------------------------------------------------------------------

#[async_trait]
pub trait MemoryService: Send + Sync {
    async fn store(&self, entry: MemoryEntry) -> Result<MemoryId>;
    async fn recall(&self, query: MemoryQuery) -> Result<Vec<ScoredMemory>>;
    async fn update(&self, update: MemoryUpdate) -> Result<()>;
    async fn delete(&self, id: MemoryId) -> Result<()>;
    async fn toggle_pin(&self, id: MemoryId, pinned: bool) -> Result<()>;
    async fn get(&self, id: MemoryId) -> Result<Option<MemoryEntry>>;
    async fn list(
        &self,
        filter: MemoryFilter,
        pagination: Pagination,
    ) -> Result<PagedResult<MemoryEntry>>;
    async fn stats(&self, agent_id: Option<AgentId>) -> Result<MemoryStats>;
}

#[async_trait]
pub trait WorkingMemoryManager: Send + Sync {
    async fn assemble_context(
        &self,
        agent_id: &AgentId,
        conversation: &Conversation,
        user_input: &str,
    ) -> Result<AssembledContext>;

    async fn compress_if_needed(
        &self,
        agent_id: &AgentId,
        conversation: &mut Conversation,
    ) -> Result<bool>;
}

#[async_trait]
pub trait MemoryExtractor: Send + Sync {
    async fn extract(
        &self,
        agent_id: &AgentId,
        messages: &[crate::llm::Message],
    ) -> Result<Vec<MemoryCandidate>>;
}

#[async_trait]
pub trait DecayEngine: Send + Sync {
    async fn run_decay(&self) -> Result<DecayReport>;
    async fn run_consolidation(&self) -> Result<ConsolidationReport>;
}

// ---------------------------------------------------------------------------
// LLM trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: CompletionRequest) -> Result<LlmResponse>;

    async fn stream(
        &self,
        request: CompletionRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk>> + Send>>>;

    /// Test connectivity to the provider.
    async fn test_connection(&self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Security trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait SecurityService: Send + Sync {
    /// Check if an agent has a given capability.
    async fn check_capability(
        &self,
        agent_id: &AgentId,
        capability: Capability,
    ) -> Result<SecurityDecision>;

    /// Run DLP scan on content.
    async fn scan_dlp(&self, content: &str, direction: crate::security::DataDirection)
        -> Result<DlpResult>;

    /// Check if a network request is allowed.
    async fn check_network(&self, url: &str) -> Result<SecurityDecision>;

    /// Check if a file path is allowed.
    async fn check_path(&self, path: &str) -> Result<SecurityDecision>;
}

// ---------------------------------------------------------------------------
// Vault trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait VaultService: Send + Sync {
    async fn create_snapshot(
        &self,
        agent_id: Option<AgentId>,
        task_id: Option<TaskId>,
        description: Option<String>,
    ) -> Result<VaultSnapshot>;

    async fn list_snapshots(&self) -> Result<Vec<VaultSnapshot>>;

    async fn diff_preview(&self, snapshot_id: SnapshotId) -> Result<DiffPreview>;

    async fn rollback(&self, snapshot_id: SnapshotId) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Knowledge trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait KnowledgeService: Send + Sync {
    async fn search(&self, query: SearchQuery) -> Result<Vec<SearchResult>>;

    async fn add_source(&self, path: String, agent_id: Option<AgentId>) -> Result<KnowledgeSourceId>;

    async fn remove_source(&self, source_id: KnowledgeSourceId) -> Result<()>;
}

// ---------------------------------------------------------------------------
// Config trait
// ---------------------------------------------------------------------------

#[async_trait]
pub trait ConfigService: Send + Sync {
    async fn load(&self) -> Result<crate::config::ClawxConfig>;
    async fn reload(&self) -> Result<crate::config::ClawxConfig>;
}

// ---------------------------------------------------------------------------
// v0.2 placeholder traits
// ---------------------------------------------------------------------------

/// Task registry port for proactive task scheduling (v0.2).
#[async_trait]
pub trait TaskRegistryPort: Send + Sync {}

/// Notification delivery port (v0.2).
#[async_trait]
pub trait NotificationPort: Send + Sync {}
