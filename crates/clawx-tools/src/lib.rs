//! clawx-tools — built-in agent tools + registry.
//!
//! Provides the `Tool` trait, `ToolRegistry`, and `ToolExecCtx` that power
//! the agent-loop tool-use iteration. Concrete tools live in sibling modules
//! (`fs`, `shell`). Approval and sandbox helpers live in `approval` and
//! `sandbox_profile`.

pub mod approval;
pub mod fs;
pub mod sandbox_profile;
pub mod shell;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::AgentId;
use clawx_types::llm::ToolDefinition;
use serde::{Deserialize, Serialize};

pub use approval::{ApprovalDecision, ApprovalPort, AutoApprovalGate};

/// Outcome of a tool invocation. `content` is the string we hand back to
/// the LLM as a `tool_result` block; `is_error` flips the block's is_error flag.
#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutcome {
    pub fn ok(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: false,
        }
    }
    pub fn err(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            is_error: true,
        }
    }
}

/// Execution context passed to every tool call. Carries workspace root,
/// agent id, and a handle to the approval port.
#[derive(Clone)]
pub struct ToolExecCtx {
    pub agent_id: AgentId,
    /// Canonicalized workspace directory. All relative paths resolve here,
    /// and no tool may write outside this tree.
    pub workspace: PathBuf,
    pub approval: Arc<dyn ApprovalPort>,
}

/// A single callable tool.
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn invoke(&self, ctx: &ToolExecCtx, arguments: serde_json::Value) -> Result<ToolOutcome>;
}

/// Registry: maps tool name → `Arc<dyn Tool>`. Deterministic iteration order
/// via `BTreeMap` would be nicer but `HashMap` is fine since the LLM sees the
/// list via `definitions()` which is sorted.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.definition().name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// All tool definitions, sorted by name for stable prompts.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<_> = self.tools.values().map(|t| t.definition()).collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
    pub fn len(&self) -> usize {
        self.tools.len()
    }
}

/// Helper: deserialize a tool's JSON arguments into its concrete params struct.
pub fn parse_args<T: for<'de> Deserialize<'de>>(
    tool_name: &str,
    args: serde_json::Value,
) -> Result<T> {
    serde_json::from_value(args)
        .map_err(|e| ClawxError::Tool(format!("tool {}: invalid arguments: {}", tool_name, e)))
}

/// Resolve `path_arg` against `workspace` and verify it cannot escape.
///
/// Two independent checks, both must pass:
///   1. Lexical: reject any `..` component and require the joined path to
///      start with `workspace` — this covers absolute paths and traversal.
///   2. Canonical: canonicalize the deepest existing ancestor of the
///      resolved path and require it still starts with the canonicalized
///      workspace — this catches pre-existing symlinks inside the tree
///      that point outside (e.g. `<ws>/inner -> /etc`).
///
/// The canonical check deliberately only descends as far as the filesystem
/// goes today, so paths that do not yet exist (fresh `fs_mkdir`, `fs_write`)
/// are still allowed.
pub fn resolve_in_workspace(workspace: &std::path::Path, path_arg: &str) -> Result<PathBuf> {
    let p = std::path::Path::new(path_arg);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        workspace.join(p)
    };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            std::path::Component::ParentDir => {
                return Err(ClawxError::Tool(format!(
                    "path escapes workspace via '..': {}",
                    path_arg
                )));
            }
            _ => out.push(c.as_os_str()),
        }
    }
    if !out.starts_with(workspace) {
        return Err(ClawxError::Tool(format!(
            "path outside workspace: {}",
            out.display()
        )));
    }

    // Canonical symlink check — walk up to the deepest existing ancestor.
    let ws_canon = workspace.canonicalize().map_err(|e| {
        ClawxError::Tool(format!(
            "workspace {} is not accessible: {}",
            workspace.display(),
            e
        ))
    })?;
    let mut probe: &std::path::Path = &out;
    while !probe.exists() {
        match probe.parent() {
            Some(parent) if parent != probe => probe = parent,
            _ => return Ok(out), // nothing exists yet: lexical check stands
        }
    }
    let ancestor_canon = probe
        .canonicalize()
        .map_err(|e| ClawxError::Tool(format!("canonicalize {}: {}", probe.display(), e)))?;
    if !ancestor_canon.starts_with(&ws_canon) {
        return Err(ClawxError::Tool(format!(
            "path escapes workspace via symlink: {} -> {}",
            probe.display(),
            ancestor_canon.display(),
        )));
    }
    Ok(out)
}

/// Shared serde schema for tools that take a single `path` argument.
#[derive(Debug, Deserialize, Serialize)]
pub struct PathArgs {
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_rejects_parent_dir() {
        let dir = tempdir().unwrap();
        let err = resolve_in_workspace(dir.path(), "../etc/passwd").unwrap_err();
        assert!(format!("{err}").contains("escapes"));
    }

    #[test]
    fn resolve_accepts_subdir() {
        let dir = tempdir().unwrap();
        let got = resolve_in_workspace(dir.path(), "sub/dir").unwrap();
        assert_eq!(got, dir.path().join("sub/dir"));
    }

    #[test]
    fn resolve_rejects_absolute_outside_workspace() {
        let dir = tempdir().unwrap();
        let err = resolve_in_workspace(dir.path(), "/etc/passwd").unwrap_err();
        assert!(
            format!("{err}").contains("outside workspace"),
            "unexpected error: {err}",
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolve_rejects_symlink_escaping_workspace() {
        let dir = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let link = dir.path().join("escape_link");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let err = resolve_in_workspace(dir.path(), "escape_link/leaked.txt").unwrap_err();
        assert!(
            format!("{err}").contains("symlink"),
            "expected symlink-escape rejection, got: {err}",
        );
    }

    #[test]
    fn registry_sorts_definitions() {
        let mut r = ToolRegistry::new();
        // defer fs tool import to keep this unit test self-contained
        struct StubTool(&'static str);
        #[async_trait::async_trait]
        impl Tool for StubTool {
            fn definition(&self) -> ToolDefinition {
                ToolDefinition {
                    name: self.0.into(),
                    description: String::new(),
                    parameters: serde_json::json!({}),
                }
            }
            async fn invoke(&self, _: &ToolExecCtx, _: serde_json::Value) -> Result<ToolOutcome> {
                Ok(ToolOutcome::ok("x"))
            }
        }
        r.register(Arc::new(StubTool("b")));
        r.register(Arc::new(StubTool("a")));
        let names: Vec<String> = r.definitions().into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["a", "b"]);
    }
}
