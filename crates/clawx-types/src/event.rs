use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, EventId, TaskId};

/// The kind of event flowing through the event bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    // Agent lifecycle
    AgentStarted,
    AgentStopped,
    AgentError,

    // Task lifecycle
    TaskCreated,
    TaskStarted,
    TaskCompleted,
    TaskFailed,

    // LLM
    LlmRequestSent,
    LlmResponseReceived,
    LlmStreamChunk,

    // Memory
    MemoryStored,
    MemoryRecalled,
    MemoryEvicted,

    // Security
    SecurityDecision,
    DlpViolation,
    PromptInjection,

    // Skills / tools
    ToolInvoked,
    ToolCompleted,
    ToolFailed,

    // Channels
    ChannelMessageReceived,
    ChannelMessageSent,

    // Snapshots
    SnapshotCreated,
    SnapshotRestored,

    // System
    ConfigReloaded,
    HealthCheck,
    Shutdown,
}

/// A single event in the ClawX system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub timestamp: DateTime<Utc>,
    pub source: String,
    pub kind: EventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// SSE payload for an execution step event sent during conversation streaming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SseExecutionStep {
    pub step_no: u32,
    pub action: String,
    pub tool: String,
    pub evidence: String,
    pub result_summary: String,
}

/// SSE payload for a confirmation-required event sent during conversation streaming.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SseConfirmationRequired {
    pub step_no: u32,
    pub tool_name: String,
    pub risk_level: String,
    pub reason: String,
}

/// Filter criteria for subscribing to events.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventFilter {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kinds: Option<Vec<EventKind>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<DateTime<Utc>>,
}
