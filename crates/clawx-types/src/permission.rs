use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::autonomy::RunId;
use crate::ids::AgentId;

/// Capability dimensions for the permission system.
/// Aligned with autonomy-architecture.md §6.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityDimension {
    KnowledgeRead,
    WorkspaceWrite,
    ExternalSend,
    MemoryWrite,
    ShellExec,
}

impl std::fmt::Display for CapabilityDimension {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KnowledgeRead => write!(f, "knowledge_read"),
            Self::WorkspaceWrite => write!(f, "workspace_write"),
            Self::ExternalSend => write!(f, "external_send"),
            Self::MemoryWrite => write!(f, "memory_write"),
            Self::ShellExec => write!(f, "shell_exec"),
        }
    }
}

impl std::str::FromStr for CapabilityDimension {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "knowledge_read" => Ok(Self::KnowledgeRead),
            "workspace_write" => Ok(Self::WorkspaceWrite),
            "external_send" => Ok(Self::ExternalSend),
            "memory_write" => Ok(Self::MemoryWrite),
            "shell_exec" => Ok(Self::ShellExec),
            other => Err(format!("unknown capability dimension: {}", other)),
        }
    }
}

/// Trust level for a capability dimension (L0-L3).
/// Aligned with autonomy-architecture.md §6.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustLevel {
    /// Default, all operations need confirmation.
    L0Restricted,
    /// Low-risk reads auto-allowed.
    L1ReadTrusted,
    /// Workspace writes auto-allowed.
    L2WorkspaceTrusted,
    /// Some external sends auto-allowed.
    L3ChannelTrusted,
}

impl Default for TrustLevel {
    fn default() -> Self {
        Self::L0Restricted
    }
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::L0Restricted => write!(f, "l0_restricted"),
            Self::L1ReadTrusted => write!(f, "l1_read_trusted"),
            Self::L2WorkspaceTrusted => write!(f, "l2_workspace_trusted"),
            Self::L3ChannelTrusted => write!(f, "l3_channel_trusted"),
        }
    }
}

impl std::str::FromStr for TrustLevel {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "l0_restricted" => Ok(Self::L0Restricted),
            "l1_read_trusted" => Ok(Self::L1ReadTrusted),
            "l2_workspace_trusted" => Ok(Self::L2WorkspaceTrusted),
            "l3_channel_trusted" => Ok(Self::L3ChannelTrusted),
            other => Err(format!("unknown trust level: {}", other)),
        }
    }
}

/// Risk level of an operation.
/// Aligned with autonomy-architecture.md §6.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    Read,
    Write,
    Send,
    MemoryLow,
    MemoryHigh,
    Danger,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "read"),
            Self::Write => write!(f, "write"),
            Self::Send => write!(f, "send"),
            Self::MemoryLow => write!(f, "memory_low"),
            Self::MemoryHigh => write!(f, "memory_high"),
            Self::Danger => write!(f, "danger"),
        }
    }
}

/// Decision from the Permission Gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionDecision {
    AutoAllow,
    Confirm { reason: String },
    Deny { reason: String },
}

/// Capability scores for an agent — maps dimension to trust level.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityScores {
    #[serde(default)]
    pub knowledge_read: TrustLevel,
    #[serde(default)]
    pub workspace_write: TrustLevel,
    #[serde(default)]
    pub external_send: TrustLevel,
    #[serde(default)]
    pub memory_write: TrustLevel,
    #[serde(default)]
    pub shell_exec: TrustLevel,
}

impl CapabilityScores {
    /// Get the trust level for a given dimension.
    pub fn get(&self, dim: CapabilityDimension) -> TrustLevel {
        match dim {
            CapabilityDimension::KnowledgeRead => self.knowledge_read,
            CapabilityDimension::WorkspaceWrite => self.workspace_write,
            CapabilityDimension::ExternalSend => self.external_send,
            CapabilityDimension::MemoryWrite => self.memory_write,
            CapabilityDimension::ShellExec => self.shell_exec,
        }
    }

    /// Set the trust level for a given dimension.
    pub fn set(&mut self, dim: CapabilityDimension, level: TrustLevel) {
        match dim {
            CapabilityDimension::KnowledgeRead => self.knowledge_read = level,
            CapabilityDimension::WorkspaceWrite => self.workspace_write = level,
            CapabilityDimension::ExternalSend => self.external_send = level,
            CapabilityDimension::MemoryWrite => self.memory_write = level,
            CapabilityDimension::ShellExec => self.shell_exec = level,
        }
    }
}

/// Permission profile for an agent.
/// Aligned with `permission_profiles` table in data-model.md §2.5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionProfile {
    pub agent_id: AgentId,
    pub capability_scores: CapabilityScores,
    pub safety_incidents: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_downgraded_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A permission change event for audit.
/// Aligned with `permission_events` table in data-model.md §2.5.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionEvent {
    pub id: uuid::Uuid,
    pub agent_id: AgentId,
    pub capability: CapabilityDimension,
    pub old_level: TrustLevel,
    pub new_level: TrustLevel,
    pub reason: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run_id: Option<RunId>,
    pub created_at: DateTime<Utc>,
}
