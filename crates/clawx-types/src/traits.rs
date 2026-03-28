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
// v0.2 traits — Autonomy / Tasks
// ---------------------------------------------------------------------------

use crate::autonomy::*;
use crate::channel::{Channel, OutboundMessage};
use serde::{Deserialize, Serialize};
use crate::permission::*;
use crate::skill::{Skill, SkillManifest};

/// Task registry: CRUD and lifecycle management for proactive tasks.
#[async_trait]
pub trait TaskRegistryPort: Send + Sync {
    async fn create_task(&self, task: Task) -> Result<TaskId>;
    async fn get_task(&self, id: TaskId) -> Result<Option<Task>>;
    async fn list_tasks(&self, agent_id: Option<AgentId>, pagination: Pagination) -> Result<PagedResult<Task>>;
    async fn update_task(&self, id: TaskId, update: TaskUpdate) -> Result<()>;
    async fn delete_task(&self, id: TaskId) -> Result<()>;
    async fn update_lifecycle(&self, id: TaskId, status: TaskLifecycleStatus) -> Result<()>;

    // Triggers
    async fn add_trigger(&self, trigger: Trigger) -> Result<TriggerId>;
    async fn get_trigger(&self, id: TriggerId) -> Result<Option<Trigger>>;
    async fn list_triggers(&self, task_id: TaskId) -> Result<Vec<Trigger>>;
    async fn update_trigger(&self, id: TriggerId, update: TriggerUpdate) -> Result<()>;
    async fn delete_trigger(&self, id: TriggerId) -> Result<()>;
    async fn get_due_triggers(&self, now: chrono::DateTime<chrono::Utc>) -> Result<Vec<Trigger>>;

    // Runs
    async fn create_run(&self, run: Run) -> Result<RunId>;
    async fn get_run(&self, id: RunId) -> Result<Option<Run>>;
    async fn list_runs(&self, task_id: TaskId, pagination: Pagination) -> Result<PagedResult<Run>>;
    async fn update_run(&self, id: RunId, update: RunUpdate) -> Result<()>;
    async fn get_incomplete_runs(&self) -> Result<Vec<Run>>;

    // Feedback
    async fn record_feedback(&self, run_id: RunId, kind: FeedbackKind, reason: Option<String>) -> Result<()>;
}

/// Partial update for a task.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TaskUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_policy: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_max_steps: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_timeout_secs: Option<u32>,
}

/// Partial update for a trigger.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TriggerUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<TriggerStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_fire_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fired_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Partial update for a run.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RunUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_status: Option<RunStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tokens_used: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub steps_count: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notification_status: Option<NotificationStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

// ---------------------------------------------------------------------------
// v0.2 traits — Notification
// ---------------------------------------------------------------------------

/// Notification delivery port.
#[async_trait]
pub trait NotificationPort: Send + Sync {
    async fn send(&self, notification: TaskNotification) -> Result<()>;
    async fn query_status(&self, run_id: RunId) -> Result<Vec<TaskNotification>>;
}

// ---------------------------------------------------------------------------
// v0.2 traits — Scheduler
// ---------------------------------------------------------------------------

/// Scheduler port for time/event trigger sources.
#[async_trait]
pub trait SchedulerPort: Send + Sync {
    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// v0.2 traits — Channel
// ---------------------------------------------------------------------------

/// Channel management port for IM integrations.
#[async_trait]
pub trait ChannelPort: Send + Sync {
    async fn connect(&self, channel_id: crate::ChannelId) -> Result<()>;
    async fn disconnect(&self, channel_id: crate::ChannelId) -> Result<()>;
    async fn send_message(&self, msg: OutboundMessage) -> Result<()>;
}

/// Channel storage port.
#[async_trait]
pub trait ChannelRegistryPort: Send + Sync {
    async fn create_channel(&self, channel: Channel) -> Result<crate::ChannelId>;
    async fn get_channel(&self, id: crate::ChannelId) -> Result<Option<Channel>>;
    async fn list_channels(&self) -> Result<Vec<Channel>>;
    async fn update_channel(&self, id: crate::ChannelId, update: ChannelUpdate) -> Result<()>;
    async fn delete_channel(&self, id: crate::ChannelId) -> Result<()>;
}

/// Partial update for a channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChannelUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<crate::AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status: Option<crate::channel::ChannelStatus>,
}

// ---------------------------------------------------------------------------
// v0.2 traits — Skills
// ---------------------------------------------------------------------------

/// Skill registry port for local skill management.
#[async_trait]
pub trait SkillRegistryPort: Send + Sync {
    async fn install(&self, manifest: SkillManifest, wasm_bytes: Vec<u8>, signature: Option<String>) -> Result<crate::SkillId>;
    async fn uninstall(&self, id: crate::SkillId) -> Result<()>;
    async fn enable(&self, id: crate::SkillId) -> Result<()>;
    async fn disable(&self, id: crate::SkillId) -> Result<()>;
    async fn get(&self, id: crate::SkillId) -> Result<Option<Skill>>;
    async fn list(&self) -> Result<Vec<Skill>>;
}

// ---------------------------------------------------------------------------
// v0.2 traits — Permission Gate
// ---------------------------------------------------------------------------

/// Permission gate for checking and managing agent trust levels.
#[async_trait]
pub trait PermissionGatePort: Send + Sync {
    async fn check_permission(
        &self,
        agent_id: &AgentId,
        risk_level: RiskLevel,
    ) -> Result<PermissionDecision>;

    async fn get_profile(&self, agent_id: &AgentId) -> Result<Option<PermissionProfile>>;

    async fn update_profile(
        &self,
        agent_id: &AgentId,
        dimension: CapabilityDimension,
        new_level: TrustLevel,
        reason: String,
    ) -> Result<()>;

    async fn record_safety_incident(&self, agent_id: &AgentId) -> Result<()>;
}

// ---------------------------------------------------------------------------
// v0.2 traits — Attention Policy
// ---------------------------------------------------------------------------

/// Attention policy for notification filtering.
#[async_trait]
pub trait AttentionPolicyPort: Send + Sync {
    async fn evaluate(
        &self,
        task: &Task,
        run: &Run,
    ) -> Result<AttentionDecision>;

    async fn record_feedback(
        &self,
        task_id: TaskId,
        feedback: FeedbackKind,
    ) -> Result<()>;
}

// ---------------------------------------------------------------------------
// v0.2 traits — Prompt Injection Guard
// ---------------------------------------------------------------------------

/// Prompt injection detection (L1 defense).
#[async_trait]
pub trait PromptInjectionGuard: Send + Sync {
    /// Check content for injection attempts. Returns Ok(()) if clean,
    /// Err(ClawxError::PromptInjection) if detected.
    async fn check(&self, content: &str) -> Result<()>;
}
