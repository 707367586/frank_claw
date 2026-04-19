//! Approval port — will be filled in by Task 7.
use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::ids::AgentId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny { reason: String },
}

#[async_trait]
pub trait ApprovalPort: Send + Sync {
    async fn check(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ApprovalDecision>;
}

/// Permissive gate — allows everything. Tests and dev only.
#[derive(Debug, Default)]
pub struct AutoApprovalGate;

#[async_trait]
impl ApprovalPort for AutoApprovalGate {
    async fn check(
        &self,
        _agent_id: &AgentId,
        _tool: &str,
        _args: &serde_json::Value,
    ) -> Result<ApprovalDecision> {
        Ok(ApprovalDecision::Allow)
    }
}
