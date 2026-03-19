use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, SnapshotId, TaskId};

/// A workspace version snapshot.
/// Aligned with `vault_snapshots` table in data-model.md §2.6.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultSnapshot {
    pub id: SnapshotId,
    /// Format: clawx-{agent_id}-{task_id}-{timestamp}
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<TaskId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Disk space in bytes.
    pub disk_size: u64,
    pub created_at: DateTime<Utc>,
}

/// Type of file change in a vault snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Added,
    Modified,
    Deleted,
    Renamed,
}

impl std::fmt::Display for ChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Added => write!(f, "added"),
            Self::Modified => write!(f, "modified"),
            Self::Deleted => write!(f, "deleted"),
            Self::Renamed => write!(f, "renamed"),
        }
    }
}

impl std::str::FromStr for ChangeType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "added" => Ok(Self::Added),
            "modified" => Ok(Self::Modified),
            "deleted" => Ok(Self::Deleted),
            "renamed" => Ok(Self::Renamed),
            other => Err(format!("unknown change type: {}", other)),
        }
    }
}

/// A single file change within a vault snapshot.
/// Aligned with `vault_changes` table in data-model.md §2.6.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultChange {
    pub id: String,
    pub snapshot_id: SnapshotId,
    pub file_path: String,
    pub change_type: ChangeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_hash: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// Preview of differences for a potential rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiffPreview {
    pub snapshot: VaultSnapshot,
    pub changes: Vec<VaultChange>,
}
