//! Agent lifecycle, Agent Loop, and task dispatcher for ClawX.
//!
//! Orchestrates the core agent loop: receive task, plan, execute tools,
//! observe results, and iterate until the task is complete or a limit
//! is reached.

pub mod agent_loop;
pub mod agent_repo;
pub mod conversation_repo;
pub mod db;
pub mod dispatcher;
pub mod lifecycle;
pub mod model_repo;

use std::sync::Arc;

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
        }
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
