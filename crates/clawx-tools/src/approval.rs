//! Three-tier approval: auto / prompt / deny, per tool × path-glob.
//!
//! A `RuleApprovalGate` resolves a tool call against a prioritized
//! rule list. Matching precedence:
//!   1. Explicit rules in insertion order.
//!   2. Default rules supplied by `default_claw_code_style`.
//!
//! Modes:
//!   - `Auto`   → Allow immediately.
//!   - `Prompt` → Delegate to the `PromptGate` (typically a channel to GUI).
//!     If no gate is wired, `Pending` surfaces as Deny.
//!   - `Deny`   → Deny with the rule's reason.

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, Mutex};

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

/// What to do when a rule matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalMode {
    Auto,
    Prompt,
    Deny,
}

#[derive(Debug, Clone)]
pub struct ApprovalRule {
    pub tool: String,
    /// Optional shell-style glob over the `path` argument (if present).
    /// If `None`, matches any args for this tool.
    pub path_glob: Option<String>,
    pub mode: ApprovalMode,
}

/// Interactive prompt delegate (GUI wires this).
#[async_trait]
pub trait PromptGate: Send + Sync {
    async fn ask(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ApprovalDecision>;
}

pub struct RuleApprovalGate {
    rules: Vec<ApprovalRule>,
    prompt: Option<Arc<dyn PromptGate>>,
}

impl RuleApprovalGate {
    pub fn new() -> Self {
        Self {
            rules: vec![],
            prompt: None,
        }
    }

    /// Baseline: read/list auto; write/mkdir/shell prompt.
    pub fn default_claw_code_style() -> Self {
        Self {
            rules: vec![
                ApprovalRule {
                    tool: "fs_read".into(),
                    path_glob: None,
                    mode: ApprovalMode::Auto,
                },
                ApprovalRule {
                    tool: "fs_list".into(),
                    path_glob: None,
                    mode: ApprovalMode::Auto,
                },
                ApprovalRule {
                    tool: "fs_write".into(),
                    path_glob: None,
                    mode: ApprovalMode::Prompt,
                },
                ApprovalRule {
                    tool: "fs_mkdir".into(),
                    path_glob: None,
                    mode: ApprovalMode::Prompt,
                },
                ApprovalRule {
                    tool: "shell_exec".into(),
                    path_glob: None,
                    mode: ApprovalMode::Prompt,
                },
            ],
            prompt: None,
        }
    }

    pub fn add_rule(&mut self, rule: ApprovalRule) {
        // Newest rule wins: prepend.
        self.rules.insert(0, rule);
    }

    pub fn with_prompt(mut self, p: Arc<dyn PromptGate>) -> Self {
        self.prompt = Some(p);
        self
    }

    fn match_rule(&self, tool: &str, args: &serde_json::Value) -> Option<&ApprovalRule> {
        let path = args.get("path").and_then(|v| v.as_str());
        self.rules.iter().find(|r| {
            r.tool == tool
                && match (&r.path_glob, path) {
                    (None, _) => true,
                    (Some(_), None) => false,
                    (Some(glob), Some(p)) => glob_match(glob, p),
                }
        })
    }
}

impl Default for RuleApprovalGate {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ApprovalPort for RuleApprovalGate {
    async fn check(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ApprovalDecision> {
        let mode = self
            .match_rule(tool_name, arguments)
            .map(|r| r.mode.clone())
            .unwrap_or(ApprovalMode::Prompt);
        match mode {
            ApprovalMode::Auto => Ok(ApprovalDecision::Allow),
            ApprovalMode::Deny => Ok(ApprovalDecision::Deny {
                reason: format!("denied by rule for tool '{}'", tool_name),
            }),
            ApprovalMode::Prompt => match &self.prompt {
                Some(p) => p.ask(agent_id, tool_name, arguments).await,
                None => Ok(ApprovalDecision::Deny {
                    reason: format!(
                        "prompt required for '{}' but no prompt gate configured",
                        tool_name
                    ),
                }),
            },
        }
    }
}

/// Minimal shell-style glob: supports `*` and `?` and literal matching. Good
/// enough for path-scope rules; avoid pulling in `globset` for one call site.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn rec(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            (Some(b'?'), Some(_)) => rec(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    rec(pattern.as_bytes(), text.as_bytes())
}

/// HTTP-backed prompt delegate.
///
/// `RuleApprovalGate::with_prompt(Arc<ChannelPromptGate>)` delegates `Prompt`
/// tier decisions here. Each pending request is tracked by a random `Uuid`
/// and a `oneshot::Sender<ApprovalDecision>`. The API handler at
/// `POST /tools/approval/:id` calls `resolve(id, decision)` to unblock the
/// tool loop.
pub struct ChannelPromptGate {
    pending: Mutex<HashMap<uuid::Uuid, oneshot::Sender<ApprovalDecision>>>,
}

impl ChannelPromptGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            pending: Mutex::new(HashMap::new()),
        })
    }

    /// Called by the API handler once the user has answered.
    ///
    /// Returns `true` if the id was known (and the pending caller will now
    /// observe `decision`), `false` if the id was not pending — either
    /// expired, unknown, or already resolved.
    pub async fn resolve(&self, id: uuid::Uuid, decision: ApprovalDecision) -> bool {
        if let Some(tx) = self.pending.lock().await.remove(&id) {
            let _ = tx.send(decision);
            true
        } else {
            false
        }
    }

    /// Test-only helper: register a pending request and hand back the id +
    /// the receiver the caller awaits to observe the decision set via
    /// `resolve`. Production callers enter via `PromptGate::ask`.
    #[doc(hidden)]
    pub async fn open_request_for_test(&self) -> (uuid::Uuid, oneshot::Receiver<ApprovalDecision>) {
        let id = uuid::Uuid::new_v4();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        (id, rx)
    }
}

#[async_trait]
impl PromptGate for ChannelPromptGate {
    async fn ask(
        &self,
        _agent_id: &AgentId,
        _tool: &str,
        _args: &serde_json::Value,
    ) -> Result<ApprovalDecision> {
        let id = uuid::Uuid::new_v4();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        // TODO(phase 2): publish an event so the GUI can fetch pending prompts.
        // For now the GUI polls GET /tools/approval (to be added in GUI plan).
        match rx.await {
            Ok(d) => Ok(d),
            Err(_) => Ok(ApprovalDecision::Deny {
                reason: "prompt channel closed".into(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::ids::AgentId;

    async fn run(gate: &RuleApprovalGate, tool: &str, args: serde_json::Value) -> ApprovalDecision {
        gate.check(&AgentId::new(), tool, &args).await.unwrap()
    }

    #[tokio::test]
    async fn auto_allow_by_default_for_read_tools() {
        let gate = RuleApprovalGate::default_claw_code_style();
        assert_eq!(
            run(&gate, "fs_read", serde_json::json!({"path":"x"})).await,
            ApprovalDecision::Allow
        );
    }

    #[tokio::test]
    async fn write_tools_default_to_prompt_which_pending_gate_denies() {
        let gate = RuleApprovalGate::default_claw_code_style();
        // No interactive prompt wired in tests → Pending surfaces as Deny.
        let d = run(&gate, "fs_write", serde_json::json!({"path":"x"})).await;
        assert!(matches!(d, ApprovalDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn explicit_auto_rule_overrides_default() {
        let mut gate = RuleApprovalGate::default_claw_code_style();
        gate.add_rule(ApprovalRule {
            tool: "fs_write".into(),
            path_glob: Some("*.md".into()),
            mode: ApprovalMode::Auto,
        });
        let d = run(&gate, "fs_write", serde_json::json!({"path":"README.md"})).await;
        assert_eq!(d, ApprovalDecision::Allow);
    }

    #[tokio::test]
    async fn deny_rule_blocks_even_if_path_glob_matches_auto() {
        let mut gate = RuleApprovalGate::default_claw_code_style();
        gate.add_rule(ApprovalRule {
            tool: "fs_write".into(),
            path_glob: Some("secrets/*".into()),
            mode: ApprovalMode::Deny,
        });
        let d = run(
            &gate,
            "fs_write",
            serde_json::json!({"path":"secrets/.env"}),
        )
        .await;
        assert!(matches!(d, ApprovalDecision::Deny { .. }));
    }

    #[test]
    fn glob_basic() {
        assert!(glob_match("*.md", "README.md"));
        assert!(glob_match("secrets/*", "secrets/.env"));
        assert!(!glob_match("*.md", "main.rs"));
    }
}
