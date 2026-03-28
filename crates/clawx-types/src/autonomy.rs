use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, TaskId};

// ---------------------------------------------------------------------------
// Task
// ---------------------------------------------------------------------------

/// Unique identifier for a trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TriggerId(pub uuid::Uuid);

impl TriggerId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for TriggerId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TriggerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for TriggerId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(Self)
    }
}

/// Unique identifier for a task run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub uuid::Uuid);

impl RunId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for RunId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for RunId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        uuid::Uuid::parse_str(s).map(Self)
    }
}

/// How a task was created.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskSourceKind {
    Conversation,
    Manual,
    Suggestion,
    Imported,
}

impl std::fmt::Display for TaskSourceKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conversation => write!(f, "conversation"),
            Self::Manual => write!(f, "manual"),
            Self::Suggestion => write!(f, "suggestion"),
            Self::Imported => write!(f, "imported"),
        }
    }
}

impl std::str::FromStr for TaskSourceKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "conversation" => Ok(Self::Conversation),
            "manual" => Ok(Self::Manual),
            "suggestion" => Ok(Self::Suggestion),
            "imported" => Ok(Self::Imported),
            other => Err(format!("unknown task source kind: {}", other)),
        }
    }
}

/// Lifecycle status of a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskLifecycleStatus {
    Active,
    Paused,
    Archived,
}

impl std::fmt::Display for TaskLifecycleStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
            Self::Archived => write!(f, "archived"),
        }
    }
}

impl std::str::FromStr for TaskLifecycleStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            "archived" => Ok(Self::Archived),
            other => Err(format!("unknown task lifecycle status: {}", other)),
        }
    }
}

/// A proactive task definition — describes "what to do".
/// Aligned with `tasks` table in data-model.md §2.5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: TaskId,
    pub agent_id: AgentId,
    pub name: String,
    pub goal: String,
    pub source_kind: TaskSourceKind,
    pub lifecycle_status: TaskLifecycleStatus,
    pub default_max_steps: u32,
    pub default_timeout_secs: u32,
    /// JSON: quiet_hours, cooldown, digest, etc.
    pub notification_policy: serde_json::Value,
    pub suppression_state: SuppressionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Suppression state for a task's notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuppressionState {
    Normal,
    Cooldown,
    PausedByFeedback,
}

impl Default for SuppressionState {
    fn default() -> Self {
        Self::Normal
    }
}

impl std::fmt::Display for SuppressionState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "normal"),
            Self::Cooldown => write!(f, "cooldown"),
            Self::PausedByFeedback => write!(f, "paused_by_feedback"),
        }
    }
}

impl std::str::FromStr for SuppressionState {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "normal" => Ok(Self::Normal),
            "cooldown" => Ok(Self::Cooldown),
            "paused_by_feedback" => Ok(Self::PausedByFeedback),
            other => Err(format!("unknown suppression state: {}", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Trigger
// ---------------------------------------------------------------------------

/// What kind of trigger fires a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerKind {
    /// Cron or one-time time trigger.
    Time,
    /// System event (FSEvents, disk alert, network change).
    Event,
    /// Context-aware trigger (v0.3+, reserved).
    Context,
    /// Policy/rule match trigger (v0.3+, reserved).
    Policy,
}

impl std::fmt::Display for TriggerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Time => write!(f, "time"),
            Self::Event => write!(f, "event"),
            Self::Context => write!(f, "context"),
            Self::Policy => write!(f, "policy"),
        }
    }
}

impl std::str::FromStr for TriggerKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "time" => Ok(Self::Time),
            "event" => Ok(Self::Event),
            "context" => Ok(Self::Context),
            "policy" => Ok(Self::Policy),
            other => Err(format!("unknown trigger kind: {}", other)),
        }
    }
}

/// A trigger attached to a task — describes "when to do it".
/// Aligned with `task_triggers` table in data-model.md §2.5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Trigger {
    pub id: TriggerId,
    pub task_id: TaskId,
    pub trigger_kind: TriggerKind,
    /// JSON: cron expression, event filter, etc.
    pub trigger_config: serde_json::Value,
    pub status: TriggerStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_fire_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fired_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerStatus {
    Active,
    Paused,
}

impl std::fmt::Display for TriggerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Paused => write!(f, "paused"),
        }
    }
}

impl std::str::FromStr for TriggerStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "paused" => Ok(Self::Paused),
            other => Err(format!("unknown trigger status: {}", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Run
// ---------------------------------------------------------------------------

/// Status of a task run (execution instance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Planning,
    Running,
    WaitingConfirmation,
    Completed,
    Failed,
    Interrupted,
}

impl std::fmt::Display for RunStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Queued => write!(f, "queued"),
            Self::Planning => write!(f, "planning"),
            Self::Running => write!(f, "running"),
            Self::WaitingConfirmation => write!(f, "waiting_confirmation"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Interrupted => write!(f, "interrupted"),
        }
    }
}

impl std::str::FromStr for RunStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "queued" => Ok(Self::Queued),
            "planning" => Ok(Self::Planning),
            "running" => Ok(Self::Running),
            "waiting_confirmation" => Ok(Self::WaitingConfirmation),
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            "interrupted" => Ok(Self::Interrupted),
            other => Err(format!("unknown run status: {}", other)),
        }
    }
}

/// A single execution instance of a task.
/// Aligned with `task_runs` table in data-model.md §2.5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub id: RunId,
    pub task_id: TaskId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger_id: Option<TriggerId>,
    pub idempotency_key: String,
    pub run_status: RunStatus,
    pub attempt: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_owner: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lease_expires_at: Option<DateTime<Utc>>,
    /// JSON: steps completed, outputs, pending confirmation.
    pub checkpoint: serde_json::Value,
    pub tokens_used: u64,
    pub steps_count: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_kind: Option<FeedbackKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feedback_reason: Option<String>,
    pub notification_status: NotificationStatus,
    pub triggered_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// User feedback on a task run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackKind {
    Accepted,
    Ignored,
    Rejected,
    MuteForever,
    ReduceFrequency,
}

impl std::fmt::Display for FeedbackKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Accepted => write!(f, "accepted"),
            Self::Ignored => write!(f, "ignored"),
            Self::Rejected => write!(f, "rejected"),
            Self::MuteForever => write!(f, "mute_forever"),
            Self::ReduceFrequency => write!(f, "reduce_frequency"),
        }
    }
}

impl std::str::FromStr for FeedbackKind {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "accepted" => Ok(Self::Accepted),
            "ignored" => Ok(Self::Ignored),
            "rejected" => Ok(Self::Rejected),
            "mute_forever" => Ok(Self::MuteForever),
            "reduce_frequency" => Ok(Self::ReduceFrequency),
            other => Err(format!("unknown feedback kind: {}", other)),
        }
    }
}

/// Notification delivery status for a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationStatus {
    Pending,
    Sent,
    Failed,
    Suppressed,
}

impl Default for NotificationStatus {
    fn default() -> Self {
        Self::Pending
    }
}

impl std::fmt::Display for NotificationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Sent => write!(f, "sent"),
            Self::Failed => write!(f, "failed"),
            Self::Suppressed => write!(f, "suppressed"),
        }
    }
}

impl std::str::FromStr for NotificationStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "sent" => Ok(Self::Sent),
            "failed" => Ok(Self::Failed),
            "suppressed" => Ok(Self::Suppressed),
            other => Err(format!("unknown notification status: {}", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Execution Step (structured output for GUI/audit)
// ---------------------------------------------------------------------------

/// A single step in a multi-step execution.
/// Aligned with autonomy-architecture.md §3.4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    pub step_no: u32,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_summary: Option<String>,
}

/// Intent category from the intent evaluator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IntentCategory {
    /// Single-turn answer.
    Simple,
    /// Needs 1 tool call.
    Assisted,
    /// Needs 2+ steps, enter Executor.
    MultiStep,
}

impl std::fmt::Display for IntentCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Simple => write!(f, "simple"),
            Self::Assisted => write!(f, "assisted"),
            Self::MultiStep => write!(f, "multi_step"),
        }
    }
}

impl std::str::FromStr for IntentCategory {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "simple" => Ok(Self::Simple),
            "assisted" => Ok(Self::Assisted),
            "multi_step" => Ok(Self::MultiStep),
            other => Err(format!("unknown intent category: {}", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Attention Policy
// ---------------------------------------------------------------------------

/// Decision from the Attention Policy engine.
/// Aligned with autonomy-architecture.md §5.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionDecision {
    /// Send notification immediately.
    SendNow,
    /// Defer to a digest batch.
    SendDigest,
    /// Only record, don't push.
    StoreOnly,
    /// Suppress entirely.
    Suppress,
}

impl std::fmt::Display for AttentionDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SendNow => write!(f, "send_now"),
            Self::SendDigest => write!(f, "send_digest"),
            Self::StoreOnly => write!(f, "store_only"),
            Self::Suppress => write!(f, "suppress"),
        }
    }
}

impl std::str::FromStr for AttentionDecision {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "send_now" => Ok(Self::SendNow),
            "send_digest" => Ok(Self::SendDigest),
            "store_only" => Ok(Self::StoreOnly),
            "suppress" => Ok(Self::Suppress),
            other => Err(format!("unknown attention decision: {}", other)),
        }
    }
}

// ---------------------------------------------------------------------------
// Task Notification record
// ---------------------------------------------------------------------------

/// Unique identifier for a task notification record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskNotificationId(pub uuid::Uuid);

impl TaskNotificationId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4())
    }
}

impl Default for TaskNotificationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskNotificationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Notification delivery record for a run.
/// Aligned with `task_notifications` table in data-model.md §2.5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskNotification {
    pub id: TaskNotificationId,
    pub run_id: RunId,
    pub channel_kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    pub delivery_status: DeliveryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suppression_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delivered_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Delivery status for a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryStatus {
    Pending,
    Sent,
    Failed,
    Suppressed,
    DigestQueued,
}

impl std::fmt::Display for DeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Sent => write!(f, "sent"),
            Self::Failed => write!(f, "failed"),
            Self::Suppressed => write!(f, "suppressed"),
            Self::DigestQueued => write!(f, "digest_queued"),
        }
    }
}

impl std::str::FromStr for DeliveryStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "sent" => Ok(Self::Sent),
            "failed" => Ok(Self::Failed),
            "suppressed" => Ok(Self::Suppressed),
            "digest_queued" => Ok(Self::DigestQueued),
            other => Err(format!("unknown delivery status: {}", other)),
        }
    }
}
