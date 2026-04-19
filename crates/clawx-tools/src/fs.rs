//! Filesystem tools: read, write, mkdir, list.
//!
//! Every path is resolved via `resolve_in_workspace`; all IO happens under
//! `tokio::fs` to stay on the async runtime. Errors are returned as
//! `ToolOutcome::err` (non-fatal) so the LLM can see and retry.

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::llm::ToolDefinition;
use serde::Deserialize;

use crate::{
    approval::ApprovalDecision, parse_args, resolve_in_workspace, Tool, ToolExecCtx, ToolOutcome,
};

// ---------------------------------------------------------------- fs_read
#[derive(Debug)]
pub struct FsReadTool;

#[derive(Debug, Deserialize)]
struct FsReadArgs {
    path: String,
    #[serde(default)]
    max_bytes: Option<usize>,
}

#[async_trait]
impl Tool for FsReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_read".into(),
            description: "Read the UTF-8 contents of a file in the workspace. \
                          Returns an error if the file is missing or not UTF-8."
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative or absolute path inside workspace." },
                    "max_bytes": { "type": "integer", "description": "Optional read cap (default 1 MiB)." }
                },
                "required": ["path"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: FsReadArgs = parse_args("fs_read", args)?;
        if let ApprovalDecision::Deny { reason } = ctx
            .approval
            .check(
                &ctx.agent_id,
                "fs_read",
                &serde_json::json!({"path": a.path}),
            )
            .await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        let cap = a.max_bytes.unwrap_or(1024 * 1024);
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let truncated = &bytes[..bytes.len().min(cap)];
                match std::str::from_utf8(truncated) {
                    Ok(s) => Ok(ToolOutcome::ok(s.to_string())),
                    Err(_) => Ok(ToolOutcome::err(format!(
                        "file is not UTF-8: {}",
                        path.display()
                    ))),
                }
            }
            Err(e) => Ok(ToolOutcome::err(format!("read {}: {}", path.display(), e))),
        }
    }
}

// ---------------------------------------------------------------- fs_write
#[derive(Debug)]
pub struct FsWriteTool;

#[derive(Debug, Deserialize)]
struct FsWriteArgs {
    path: String,
    content: String,
    #[serde(default)]
    create_parents: Option<bool>,
}

#[async_trait]
impl Tool for FsWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_write".into(),
            description: "Write UTF-8 `content` to `path` in the workspace, creating \
                          or overwriting the file. Set `create_parents:true` to \
                          create missing parent directories."
                .into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{
                    "path":{"type":"string"},
                    "content":{"type":"string"},
                    "create_parents":{"type":"boolean","default":false}
                },
                "required":["path","content"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: FsWriteArgs = parse_args("fs_write", args)?;
        if let ApprovalDecision::Deny { reason } = ctx
            .approval
            .check(
                &ctx.agent_id,
                "fs_write",
                &serde_json::json!({"path": a.path}),
            )
            .await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        if a.create_parents.unwrap_or(false) {
            if let Some(parent) = path.parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return Ok(ToolOutcome::err(format!(
                        "mkdir {}: {}",
                        parent.display(),
                        e
                    )));
                }
            }
        }
        match tokio::fs::write(&path, a.content.as_bytes()).await {
            Ok(()) => Ok(ToolOutcome::ok(format!(
                "wrote {} bytes to {}",
                a.content.len(),
                path.display()
            ))),
            Err(e) => Ok(ToolOutcome::err(format!("write {}: {}", path.display(), e))),
        }
    }
}

// ---------------------------------------------------------------- fs_mkdir
#[derive(Debug)]
pub struct FsMkdirTool;

#[async_trait]
impl Tool for FsMkdirTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_mkdir".into(),
            description: "Create a directory (and missing parents) at `path`.".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"path":{"type":"string"}},
                "required":["path"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: crate::PathArgs = parse_args("fs_mkdir", args)?;
        if let ApprovalDecision::Deny { reason } = ctx
            .approval
            .check(
                &ctx.agent_id,
                "fs_mkdir",
                &serde_json::json!({"path": a.path}),
            )
            .await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        match tokio::fs::create_dir_all(&path).await {
            Ok(()) => Ok(ToolOutcome::ok(format!("created {}", path.display()))),
            Err(e) => Ok(ToolOutcome::err(format!("mkdir {}: {}", path.display(), e))),
        }
    }
}

// ---------------------------------------------------------------- fs_list
#[derive(Debug)]
pub struct FsListTool;

#[async_trait]
impl Tool for FsListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_list".into(),
            description: "List entries (one per line) in the given directory \
                          inside the workspace. Directories suffixed with '/'."
                .into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"path":{"type":"string"}},
                "required":["path"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: crate::PathArgs = parse_args("fs_list", args)?;
        if let ApprovalDecision::Deny { reason } = ctx
            .approval
            .check(
                &ctx.agent_id,
                "fs_list",
                &serde_json::json!({"path": a.path}),
            )
            .await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        let mut rd = match tokio::fs::read_dir(&path).await {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutcome::err(format!("list {}: {}", path.display(), e))),
        };
        let mut names: Vec<String> = Vec::new();
        loop {
            match rd.next_entry().await {
                Ok(Some(entry)) => {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                    names.push(if is_dir { format!("{name}/") } else { name });
                }
                Ok(None) => break,
                Err(e) => return Ok(ToolOutcome::err(format!("list iter: {}", e))),
            }
        }
        names.sort();
        Ok(ToolOutcome::ok(names.join("\n")))
    }
}
