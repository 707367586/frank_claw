//! ClawX Service library surface.
//!
//! Exposes a test-friendly `Runtime` builder so integration tests can
//! assert that tools + approval gate are wired into the composition root
//! exactly as the production binary wires them. The production entry
//! point lives in `main.rs`; this module intentionally only exports the
//! test builder for now.

use std::sync::Arc;

use clawx_runtime::{db::Database, Runtime};
use clawx_tools::{
    approval::RuleApprovalGate,
    fs::{FsListTool, FsMkdirTool, FsReadTool, FsWriteTool},
    shell::ShellExecTool,
    ApprovalPort, ToolRegistry,
};

/// Build the default built-in tool registry: `fs_read`, `fs_write`,
/// `fs_mkdir`, `fs_list`, `shell_exec`. Kept in the library so the
/// production `main.rs` and the service-level smoke test register the
/// exact same surface.
pub fn build_default_tool_registry() -> ToolRegistry {
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(FsReadTool));
    reg.register(Arc::new(FsWriteTool));
    reg.register(Arc::new(FsMkdirTool));
    reg.register(Arc::new(FsListTool));
    reg.register(Arc::new(ShellExecTool::default()));
    reg
}

/// Build the default rule-based approval gate mirroring Claude Code
/// ergonomics (read/list auto, write/mkdir/shell prompt).
pub fn build_default_approval_gate() -> Arc<dyn ApprovalPort> {
    Arc::new(RuleApprovalGate::default_claw_code_style())
}

/// Build a `Runtime` with in-memory DB, stub services, and the full
/// built-in tool set + default rule-based approval gate. Used by the
/// service-level smoke test that guards the "tools are wired at boot"
/// contract.
pub async fn build_runtime_for_tests() -> anyhow::Result<Runtime> {
    let db = Database::in_memory().await?;
    let workspace = std::env::temp_dir().join("clawx-test-workspace");
    tokio::fs::create_dir_all(&workspace).await?;

    let reg = build_default_tool_registry();
    let approval = build_default_approval_gate();

    Ok(Runtime::new(
        db,
        Arc::new(clawx_llm::StubLlmProvider),
        Arc::new(clawx_memory::StubMemoryService),
        Arc::new(clawx_memory::StubWorkingMemoryManager),
        Arc::new(clawx_memory::StubMemoryExtractor),
        Arc::new(clawx_security::PermissiveSecurityGuard),
        Arc::new(clawx_vault::StubVaultService),
        Arc::new(clawx_kb::StubKnowledgeService),
        Arc::new(clawx_config::ConfigLoader::with_defaults()),
    )
    .with_tools(Arc::new(reg), approval, workspace))
}
