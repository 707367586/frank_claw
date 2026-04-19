use std::sync::Arc;

use clawx_tools::approval::AutoApprovalGate;
use clawx_tools::fs::{FsListTool, FsMkdirTool, FsReadTool, FsWriteTool};
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

#[tokio::test]
async fn mkdir_write_read_list_round_trip() {
    let dir = tempdir().unwrap();
    let c = ctx(dir.path());

    // 1) mkdir
    let r = FsMkdirTool
        .invoke(&c, serde_json::json!({"path": "sub/inner"}))
        .await
        .unwrap();
    assert!(!r.is_error, "mkdir: {}", r.content);
    assert!(dir.path().join("sub/inner").is_dir());

    // 2) write
    let r = FsWriteTool
        .invoke(
            &c,
            serde_json::json!({"path": "sub/hello.txt", "content": "hi"}),
        )
        .await
        .unwrap();
    assert!(!r.is_error, "write: {}", r.content);

    // 3) read
    let r = FsReadTool
        .invoke(&c, serde_json::json!({"path": "sub/hello.txt"}))
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("hi"));

    // 4) list
    let r = FsListTool
        .invoke(&c, serde_json::json!({"path": "sub"}))
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("hello.txt"));
    assert!(r.content.contains("inner"));
}

#[tokio::test]
async fn write_outside_workspace_rejected() {
    let dir = tempdir().unwrap();
    let c = ctx(dir.path());
    let r = FsWriteTool
        .invoke(
            &c,
            serde_json::json!({"path": "../escape.txt", "content": "x"}),
        )
        .await;
    assert!(r.is_err() || r.as_ref().unwrap().is_error);
}
