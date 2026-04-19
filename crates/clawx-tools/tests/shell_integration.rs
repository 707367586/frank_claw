use std::sync::Arc;

use clawx_tools::approval::AutoApprovalGate;
use clawx_tools::shell::ShellExecTool;
use clawx_tools::{Tool, ToolExecCtx};
use clawx_types::ids::AgentId;
use tempfile::tempdir;

fn ctx(ws: &std::path::Path) -> ToolExecCtx {
    ToolExecCtx {
        agent_id: AgentId::new(),
        workspace: ws.to_path_buf(),
        approval: Arc::new(AutoApprovalGate),
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn shell_runs_echo_in_sandbox() {
    let dir = tempdir().unwrap();
    let ws = dir.path().canonicalize().unwrap();
    let c = ctx(&ws);
    let r = ShellExecTool::default()
        .invoke(&c, serde_json::json!({"command": "echo hello-claw"}))
        .await
        .unwrap();
    assert!(!r.is_error, "stderr: {}", r.content);
    assert!(r.content.contains("hello-claw"));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn shell_blocks_write_outside_workspace() {
    let dir = tempdir().unwrap();
    let ws = dir.path().canonicalize().unwrap();
    let c = ctx(&ws);
    // Try to write to $HOME (outside the workspace); sandbox should deny.
    let r = ShellExecTool::default()
        .invoke(
            &c,
            serde_json::json!({
                "command": "touch $HOME/claw-sandbox-escape-$$"
            }),
        )
        .await
        .unwrap();
    assert!(r.is_error || r.content.contains("Operation not permitted"));
}

#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn shell_reports_unsupported() {
    let dir = tempdir().unwrap();
    let c = ctx(dir.path());
    let r = ShellExecTool::default()
        .invoke(&c, serde_json::json!({"command": "echo hi"}))
        .await
        .unwrap();
    assert!(r.is_error && r.content.contains("unsupported"));
}
