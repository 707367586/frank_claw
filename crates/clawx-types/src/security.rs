use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, AuditEntryId};

/// Capability types for the L4 permission model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Capability {
    FsRead,
    FsWrite,
    NetHttp,
    ExecShell,
    SecretInject,
}

/// Outcome of a security evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecurityDecision {
    Allow,
    Deny { reason: String },
    Escalate { reason: String },
}

/// Direction of data flow for DLP scanning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataDirection {
    Inbound,
    Outbound,
}

/// Result of a DLP (Data Loss Prevention) scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DlpResult {
    pub passed: bool,
    pub direction: DataDirection,
    #[serde(default)]
    pub violations: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redacted_content: Option<String>,
}

/// A single entry in the SHA-256 hash-chain audit log (L12).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEntry {
    pub id: AuditEntryId,
    pub timestamp: DateTime<Utc>,
    pub agent_id: AgentId,
    pub action: String,
    pub decision: SecurityDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// SHA-256 hash of the previous entry (hash chain).
    pub prev_hash: String,
    /// SHA-256 hash of this entry.
    pub hash: String,
}

/// A network whitelist entry for L6 SSRF protection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkWhitelistEntry {
    pub domain: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// Path permission for L7 path traversal protection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPermission {
    pub path: String,
    pub read: bool,
    pub write: bool,
}
