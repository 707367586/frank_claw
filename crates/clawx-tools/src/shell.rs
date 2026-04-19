//! shell_exec — run a shell command inside a macOS `sandbox-exec` profile.
//!
//! Non-macOS platforms return a `ToolOutcome::err("unsupported ...")`.
//! The profile (see `sandbox_profile`) denies network and confines file
//! writes to the workspace.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::llm::ToolDefinition;
use serde::Deserialize;
use tokio::process::Command;

use crate::{
    approval::ApprovalDecision, parse_args, sandbox_profile, Tool, ToolExecCtx, ToolOutcome,
};

#[derive(Debug, Default)]
pub struct ShellExecTool {
    /// Default timeout for a command. Override per-call via the `timeout_secs` arg.
    pub default_timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct ShellArgs {
    command: String,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(default)]
    cwd: Option<String>,
}

#[async_trait]
impl Tool for ShellExecTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell_exec".into(),
            description: "Run a shell command inside a macOS sandbox scoped to the \
                          workspace. Captures stdout+stderr. Default timeout 30s. \
                          Network is denied; file writes outside the workspace are denied."
                .into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{
                    "command":{"type":"string","description":"/bin/sh -c <command>"},
                    "timeout_secs":{"type":"integer","default":30},
                    "cwd":{"type":"string","description":"Workspace-relative cwd (default: workspace root)."}
                },
                "required":["command"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: ShellArgs = parse_args("shell_exec", args)?;
        if let ApprovalDecision::Deny { reason } = ctx
            .approval
            .check(
                &ctx.agent_id,
                "shell_exec",
                &serde_json::json!({"command": a.command}),
            )
            .await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }

        if !cfg!(target_os = "macos") {
            return Ok(ToolOutcome::err(
                "shell_exec unsupported on this platform (macOS only in phase 1)",
            ));
        }

        let cwd: PathBuf = match a.cwd {
            Some(rel) => match crate::resolve_in_workspace(&ctx.workspace, &rel) {
                Ok(p) => p,
                Err(e) => return Ok(ToolOutcome::err(e.to_string())),
            },
            None => ctx.workspace.clone(),
        };

        let profile = sandbox_profile::workspace_profile(&ctx.workspace);
        let timeout = Duration::from_secs(
            a.timeout_secs
                .unwrap_or_else(|| self.default_timeout_secs.max(30)),
        );

        // sandbox-exec -p '<profile>' /bin/sh -c '<command>'
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p")
            .arg(&profile)
            .arg("/bin/sh")
            .arg("-c")
            .arg(&a.command)
            .current_dir(&cwd)
            .kill_on_drop(true);

        let run = async {
            let out = cmd.output().await?;
            Ok::<_, std::io::Error>(out)
        };

        let output = match tokio::time::timeout(timeout, run).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Ok(ToolOutcome::err(format!("spawn: {e}"))),
            Err(_) => return Ok(ToolOutcome::err(format!("timeout after {:?}", timeout))),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let mut body = String::new();
        if !stdout.is_empty() {
            body.push_str("=== stdout ===\n");
            body.push_str(&stdout);
            if !stdout.ends_with('\n') {
                body.push('\n');
            }
        }
        if !stderr.is_empty() {
            body.push_str("=== stderr ===\n");
            body.push_str(&stderr);
        }
        let code = output.status.code().unwrap_or(-1);
        body.push_str(&format!("\n=== exit {code} ==="));
        Ok(if output.status.success() {
            ToolOutcome::ok(body)
        } else {
            ToolOutcome::err(body)
        })
    }
}
