//! Tool-use iteration driver for the agent loop.
//!
//! Responsibilities:
//! 1. Inject `ToolRegistry::definitions()` into the LLM request.
//! 2. When the LLM returns `StopReason::ToolUse`, dispatch each call,
//!    run the tool, and append a `role: Tool` message with `ToolResult`
//!    blocks back into the conversation.
//! 3. Re-query the LLM until it stops for any non-ToolUse reason,
//!    or until `max_iterations` is reached.

use std::sync::Arc;

use clawx_tools::{ToolExecCtx, ToolRegistry};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::AgentId;
use clawx_types::llm::{
    CompletionRequest, ContentBlock, LlmResponse, Message, MessageRole, StopReason, TokenUsage,
};
use clawx_types::traits::LlmProvider;
use tracing::{debug, info, warn};

pub struct ToolLoopConfig {
    pub max_iterations: u32,
    pub max_tokens: u32,
}

#[derive(Debug)]
pub struct ToolLoopOutcome {
    pub final_content: String,
    pub tool_calls_made: u32,
    pub usage: TokenUsage,
    pub messages: Vec<Message>, // for persistence
}

pub async fn run_with_tools(
    llm: Arc<dyn LlmProvider>,
    registry: Arc<ToolRegistry>,
    exec_ctx: ToolExecCtx,
    agent_id: &AgentId,
    mut messages: Vec<Message>,
    model: String,
    cfg: ToolLoopConfig,
) -> Result<ToolLoopOutcome> {
    let mut tool_calls_made: u32 = 0;
    let mut total_usage = TokenUsage::default();

    for iter in 0..cfg.max_iterations {
        let req = CompletionRequest {
            model: model.clone(),
            messages: messages.clone(),
            tools: if registry.is_empty() {
                None
            } else {
                Some(registry.definitions())
            },
            temperature: None,
            max_tokens: Some(cfg.max_tokens),
            stream: false,
        };
        debug!(%agent_id, iter, msg_count = messages.len(), "tool_loop: calling LLM");
        let resp: LlmResponse = llm.complete(req).await?;
        total_usage.prompt_tokens += resp.usage.prompt_tokens;
        total_usage.completion_tokens += resp.usage.completion_tokens;
        total_usage.total_tokens += resp.usage.total_tokens;

        let assistant_blocks = reconstruct_assistant_blocks(&resp);
        messages.push(Message {
            role: MessageRole::Assistant,
            content: resp.content.clone(),
            blocks: assistant_blocks,
            tool_call_id: None,
        });

        if resp.stop_reason != StopReason::ToolUse || resp.tool_calls.is_empty() {
            return Ok(ToolLoopOutcome {
                final_content: resp.content,
                tool_calls_made,
                usage: total_usage,
                messages,
            });
        }

        // Dispatch each tool_use in parallel-safe order (sequential for now).
        let mut result_blocks: Vec<ContentBlock> = Vec::new();
        for call in &resp.tool_calls {
            tool_calls_made += 1;
            let tool = match registry.get(&call.name) {
                Some(t) => t,
                None => {
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: call.id.clone(),
                        content: format!("unknown tool: {}", call.name),
                        is_error: true,
                    });
                    continue;
                }
            };
            info!(%agent_id, tool = %call.name, "tool_loop: invoking");
            let outcome = match tool.invoke(&exec_ctx, call.arguments.clone()).await {
                Ok(o) => o,
                Err(e) => clawx_tools::ToolOutcome::err(format!("tool panicked: {e}")),
            };
            result_blocks.push(ContentBlock::ToolResult {
                tool_use_id: call.id.clone(),
                content: outcome.content,
                is_error: outcome.is_error,
            });
        }
        messages.push(Message {
            role: MessageRole::Tool,
            content: String::new(),
            blocks: result_blocks,
            tool_call_id: None,
        });
    }

    warn!(%agent_id, "tool_loop: hit max_iterations={}", cfg.max_iterations);
    Err(ClawxError::Tool(format!(
        "exceeded max tool iterations ({}); last user-visible content may be incomplete",
        cfg.max_iterations
    )))
}

/// Rebuild the `ContentBlock` representation of an assistant turn so that
/// the follow-up request preserves the tool_use blocks in the transcript.
fn reconstruct_assistant_blocks(resp: &LlmResponse) -> Vec<ContentBlock> {
    let mut blocks: Vec<ContentBlock> = Vec::new();
    if !resp.content.is_empty() {
        blocks.push(ContentBlock::Text {
            text: resp.content.clone(),
        });
    }
    for tc in &resp.tool_calls {
        blocks.push(ContentBlock::ToolUse {
            id: tc.id.clone(),
            name: tc.name.clone(),
            input: tc.arguments.clone(),
        });
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use clawx_tools::{approval::AutoApprovalGate, Tool};
    use clawx_types::llm::{LlmStreamChunk, ToolCall, ToolDefinition};
    use std::pin::Pin;
    use std::sync::Mutex;

    struct ScriptedLlm {
        responses: Mutex<Vec<LlmResponse>>,
    }
    #[async_trait]
    impl LlmProvider for ScriptedLlm {
        async fn complete(&self, _req: CompletionRequest) -> Result<LlmResponse> {
            Ok(self.responses.lock().unwrap().remove(0))
        }
        async fn stream(
            &self,
            _req: CompletionRequest,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk>> + Send>>> {
            unimplemented!()
        }
        async fn test_connection(&self) -> Result<()> {
            Ok(())
        }
    }

    struct EchoTool;
    #[async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".into(),
                description: "echo".into(),
                parameters: serde_json::json!({"type":"object"}),
            }
        }
        async fn invoke(
            &self,
            _ctx: &ToolExecCtx,
            args: serde_json::Value,
        ) -> Result<clawx_tools::ToolOutcome> {
            Ok(clawx_tools::ToolOutcome::ok(
                args["msg"].as_str().unwrap_or("").to_string(),
            ))
        }
    }

    #[tokio::test]
    async fn loop_runs_tool_and_finalizes() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(EchoTool));
        let llm = Arc::new(ScriptedLlm {
            responses: Mutex::new(vec![
                LlmResponse {
                    content: String::new(),
                    stop_reason: StopReason::ToolUse,
                    tool_calls: vec![ToolCall {
                        id: "c1".into(),
                        name: "echo".into(),
                        arguments: serde_json::json!({"msg":"pong"}),
                    }],
                    usage: TokenUsage::default(),
                    metadata: None,
                },
                LlmResponse {
                    content: "pong".into(),
                    stop_reason: StopReason::EndTurn,
                    tool_calls: vec![],
                    usage: TokenUsage::default(),
                    metadata: None,
                },
            ]),
        });
        let exec = ToolExecCtx {
            agent_id: AgentId::new(),
            workspace: std::env::temp_dir(),
            approval: Arc::new(AutoApprovalGate),
        };
        let out = run_with_tools(
            llm,
            Arc::new(reg),
            exec.clone(),
            &exec.agent_id,
            vec![Message {
                role: MessageRole::User,
                content: "ping".into(),
                blocks: vec![],
                tool_call_id: None,
            }],
            "stub".into(),
            ToolLoopConfig {
                max_iterations: 5,
                max_tokens: 256,
            },
        )
        .await
        .unwrap();
        assert_eq!(out.final_content, "pong");
        assert_eq!(out.tool_calls_made, 1);
    }

    #[tokio::test]
    async fn loop_errors_on_max_iterations() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(EchoTool));
        // LLM keeps asking to call the tool forever.
        let infinite = (0..100)
            .map(|_| LlmResponse {
                content: String::new(),
                stop_reason: StopReason::ToolUse,
                tool_calls: vec![ToolCall {
                    id: "c1".into(),
                    name: "echo".into(),
                    arguments: serde_json::json!({"msg":"x"}),
                }],
                usage: TokenUsage::default(),
                metadata: None,
            })
            .collect();
        let llm = Arc::new(ScriptedLlm {
            responses: Mutex::new(infinite),
        });
        let exec = ToolExecCtx {
            agent_id: AgentId::new(),
            workspace: std::env::temp_dir(),
            approval: Arc::new(AutoApprovalGate),
        };
        let err = run_with_tools(
            llm,
            Arc::new(reg),
            exec.clone(),
            &exec.agent_id,
            vec![],
            "stub".into(),
            ToolLoopConfig {
                max_iterations: 3,
                max_tokens: 256,
            },
        )
        .await
        .unwrap_err();
        assert!(format!("{err}").contains("max tool iterations"));
    }
}
