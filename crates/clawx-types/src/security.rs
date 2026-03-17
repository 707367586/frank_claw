use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::AgentId;

/// Execution tier controlling the sandbox level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionTier {
    Sandboxed,
    Subprocess,
    Native,
}

/// Permissions granted for a particular execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPermissions {
    pub tier: ExecutionTier,
    pub allow_network: bool,
    pub allow_filesystem: bool,
    pub max_memory_bytes: u64,
    pub max_cpu_time_ms: u64,
    #[serde(default)]
    pub allowed_hosts: Vec<String>,
    #[serde(default)]
    pub allowed_paths: Vec<String>,
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
    #[serde(default)]
    pub redacted_content: Option<String>,
}

/// Result of a prompt-injection scan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InjectionScanResult {
    pub is_injection: bool,
    pub score: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// Record of an auditable security event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub timestamp: DateTime<Utc>,
    pub agent_id: AgentId,
    pub action: String,
    pub decision: SecurityDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}
