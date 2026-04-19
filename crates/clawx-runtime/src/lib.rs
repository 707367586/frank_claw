//! Agent lifecycle, Agent Loop, and task dispatcher for ClawX.
//!
//! Orchestrates the core agent loop: receive task, plan, execute tools,
//! observe results, and iterate until the task is complete or a limit
//! is reached.

pub mod agent_loop;
pub mod agent_repo;
pub mod autonomy;
pub mod channel_handler;
pub mod channel_repo;
pub mod conversation_repo;
pub mod db;
pub mod dispatcher;
pub mod lifecycle;
pub mod model_repo;
pub mod notification_repo;
pub mod permission_repo;
pub mod run_recovery;
pub mod seed;
pub mod skill_loader;
pub mod skill_repo;
pub mod task_repo;
pub mod tool_loop;

use std::sync::Arc;

use clawx_tools::{ApprovalPort, ToolRegistry};
use clawx_types::traits::*;

/// The Runtime holds all service trait objects and wires them together.
/// This is the composition root for the agent subsystem.
#[derive(Clone)]
pub struct Runtime {
    pub db: db::Database,
    pub llm: Arc<dyn LlmProvider>,
    pub memory: Arc<dyn MemoryService>,
    pub working_memory: Arc<dyn WorkingMemoryManager>,
    pub memory_extractor: Arc<dyn MemoryExtractor>,
    pub security: Arc<dyn SecurityService>,
    pub vault: Arc<dyn VaultService>,
    pub knowledge: Arc<dyn KnowledgeService>,
    pub config: Arc<dyn ConfigService>,
    /// v0.2: Task registry for autonomous execution.
    pub task_registry: Option<Arc<dyn TaskRegistryPort>>,
    /// v0.2: Permission gate for risk-based access control.
    pub permission_gate: Option<Arc<dyn PermissionGatePort>>,
    /// Built-in tool registry. When `None`, `run_turn` degrades to a plain
    /// single-call LLM request (legacy behavior).
    pub tools: Option<Arc<ToolRegistry>>,
    /// Approval gate for tool invocations. Required whenever `tools` is set.
    pub approval: Option<Arc<dyn ApprovalPort>>,
    /// Workspace root. All tool IO resolves here. Required when `tools` is set.
    pub workspace: Option<std::path::PathBuf>,
    /// Max tool iterations per turn (safety brake). Default 10.
    pub max_tool_iterations: u32,
}

impl Runtime {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        db: db::Database,
        llm: Arc<dyn LlmProvider>,
        memory: Arc<dyn MemoryService>,
        working_memory: Arc<dyn WorkingMemoryManager>,
        memory_extractor: Arc<dyn MemoryExtractor>,
        security: Arc<dyn SecurityService>,
        vault: Arc<dyn VaultService>,
        knowledge: Arc<dyn KnowledgeService>,
        config: Arc<dyn ConfigService>,
    ) -> Self {
        Self {
            db,
            llm,
            memory,
            working_memory,
            memory_extractor,
            security,
            vault,
            knowledge,
            config,
            task_registry: None,
            permission_gate: None,
            tools: None,
            approval: None,
            workspace: None,
            max_tool_iterations: 10,
        }
    }

    /// Set the task registry (v0.2 autonomy).
    pub fn with_task_registry(mut self, registry: Arc<dyn TaskRegistryPort>) -> Self {
        self.task_registry = Some(registry);
        self
    }

    /// Set the permission gate (v0.2 autonomy).
    pub fn with_permission_gate(mut self, gate: Arc<dyn PermissionGatePort>) -> Self {
        self.permission_gate = Some(gate);
        self
    }

    /// Wire the built-in tool registry + approval gate + workspace root.
    /// All three must be provided together — when any is missing, the agent
    /// loop degrades to a single-call LLM request (legacy behavior).
    pub fn with_tools(
        mut self,
        tools: Arc<ToolRegistry>,
        approval: Arc<dyn ApprovalPort>,
        workspace: std::path::PathBuf,
    ) -> Self {
        self.tools = Some(tools);
        self.approval = Some(approval);
        self.workspace = Some(workspace);
        self
    }

    /// Override the per-turn tool iteration cap.
    pub fn with_max_tool_iterations(mut self, n: u32) -> Self {
        self.max_tool_iterations = n;
        self
    }
}

impl std::fmt::Debug for Runtime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Runtime").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_runtime() -> Runtime {
        Runtime::new(
            db::Database::in_memory().await.unwrap(),
            Arc::new(clawx_llm::StubLlmProvider),
            Arc::new(clawx_memory::StubMemoryService),
            Arc::new(clawx_memory::StubWorkingMemoryManager),
            Arc::new(clawx_memory::StubMemoryExtractor),
            Arc::new(clawx_security::PermissiveSecurityGuard),
            Arc::new(clawx_vault::StubVaultService),
            Arc::new(clawx_kb::StubKnowledgeService),
            Arc::new(clawx_config::ConfigLoader::with_defaults()),
        )
    }

    #[tokio::test]
    async fn runtime_can_be_constructed() {
        let rt = make_runtime().await;
        let _ = &rt.llm;
        let _ = &rt.memory;
        let _ = &rt.working_memory;
        let _ = &rt.security;
        let _ = &rt.vault;
        let _ = &rt.knowledge;
        let _ = &rt.config;
        let _ = &rt.db;
    }

    #[tokio::test]
    async fn runtime_is_cloneable() {
        let rt = make_runtime().await;
        let _rt2 = rt.clone();
    }

    #[tokio::test]
    async fn runtime_is_debuggable() {
        let rt = make_runtime().await;
        let debug = format!("{:?}", rt);
        assert!(debug.contains("Runtime"));
    }
}
