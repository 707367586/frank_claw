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

/// Canonicalize `path_arg` under `workspace`. Returns `Err` if it escapes.
pub fn resolve_in_workspace(workspace: &std::path::Path, path_arg: &str) -> Result<PathBuf> {
    let p = std::path::Path::new(path_arg);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        workspace.join(p)
    };
    // Disallow `..` components outright. We don't call `canonicalize` because
    // the path may not exist yet (mkdir, write). Instead, walk the components.
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
    use std::path::PathBuf;

    #[test]
    fn resolve_rejects_parent_dir() {
        let ws = PathBuf::from("/ws");
        let err = resolve_in_workspace(&ws, "../etc/passwd").unwrap_err();
        assert!(format!("{err}").contains("escapes"));
    }

    #[test]
    fn resolve_accepts_subdir() {
        let ws = PathBuf::from("/ws");
        let got = resolve_in_workspace(&ws, "sub/dir").unwrap();
        assert_eq!(got, PathBuf::from("/ws/sub/dir"));
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
