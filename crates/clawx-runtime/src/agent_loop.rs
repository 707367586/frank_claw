//! The core agent loop: receive message → assemble context → call LLM → return response.
//!
//! v0.2: Intent evaluation determines if a request needs multi-step execution.
//! Simple/Assisted requests are handled inline; MultiStep requests are delegated
//! to the TaskExecutor via Task/Run creation.

use clawx_types::agent::*;
use clawx_types::autonomy::*;
use clawx_types::error::Result;
use clawx_types::ids::*;
use clawx_types::llm::*;
use clawx_types::memory::*;
use tracing::{debug, info, warn};

use crate::autonomy::executor::IntentEvaluator;
use crate::Runtime;

/// Run a single turn of the agent loop.
///
/// 1. Evaluate intent (simple / assisted / multi_step)
/// 2. Assemble context (system prompt + memories + conversation history)
/// 3. Call LLM
/// 4. For multi_step: create Task + Run, return execution plan summary
/// 5. Extract memories from the conversation
/// 6. Return the response
pub async fn run_turn(
    runtime: &Runtime,
    agent_id: &AgentId,
    conversation: &Conversation,
    user_input: &str,
) -> Result<AgentResponse> {
    info!(%agent_id, "agent loop: running turn");

    // Step 0: Evaluate intent complexity
    let intent = IntentEvaluator::evaluate(user_input);
    debug!(%agent_id, ?intent, "intent evaluated");

    // For multi-step requests, delegate to the autonomy system if available
    if intent == IntentCategory::MultiStep {
        if let Some(ref task_registry) = runtime.task_registry {
            info!(%agent_id, "multi-step intent detected, creating task");
            return handle_multi_step(runtime, agent_id, user_input, task_registry.clone()).await;
        }
        // Fall through to normal LLM call if task_registry not configured
        debug!(%agent_id, "task_registry not available, falling back to single-turn");
    }

    // Step 1: Assemble context via WorkingMemoryManager
    let ctx = runtime
        .working_memory
        .assemble_context(agent_id, conversation, user_input)
        .await?;

    debug!(
        total_tokens = ctx.total_tokens,
        memory_count = ctx.recalled_memories.len(),
        "assembled context"
    );

    // Step 2: Build completion request — inject recalled memories into system prompt.
    let mut messages = Vec::new();
    let system_prompt = if ctx.recalled_memories.is_empty() {
        ctx.system_prompt
    } else {
        let memory_block: String = ctx
            .recalled_memories
            .iter()
            .map(|m| format!("- {}", m.entry.summary))
            .collect::<Vec<_>>()
            .join("\n");
        format!(
            "{}\n\n## Relevant memories\n{}",
            ctx.system_prompt, memory_block
        )
    };
    if !system_prompt.is_empty() {
        messages.push(Message {
            role: MessageRole::System,
            content: system_prompt,
            blocks: vec![],
            tool_call_id: None,
        });
    }
    messages.extend(ctx.conversation_history);
    messages.push(Message {
        role: MessageRole::User,
        content: user_input.to_string(),
        blocks: vec![],
        tool_call_id: None,
    });

    // Step 3: With tools → iterate. Without tools → legacy single call.
    let (final_content, tool_calls_made, usage) =
        match (&runtime.tools, &runtime.approval, &runtime.workspace) {
            (Some(tools), Some(approval), Some(workspace)) => {
                let exec_ctx = clawx_tools::ToolExecCtx {
                    agent_id: *agent_id,
                    workspace: workspace.clone(),
                    approval: approval.clone(),
                };
                let outcome = crate::tool_loop::run_with_tools(
                    runtime.llm.clone(),
                    tools.clone(),
                    exec_ctx,
                    agent_id,
                    messages.clone(),
                    "default".into(),
                    crate::tool_loop::ToolLoopConfig {
                        max_iterations: runtime.max_tool_iterations,
                        max_tokens: 4096,
                    },
                )
                .await?;
                (
                    outcome.final_content,
                    outcome.tool_calls_made,
                    outcome.usage,
                )
            }
            _ => {
                let request = CompletionRequest {
                    model: "default".to_string(),
                    messages: messages.clone(),
                    tools: None,
                    temperature: None,
                    max_tokens: Some(4096),
                    stream: false,
                };
                let response = runtime.llm.complete(request).await?;
                (
                    response.content,
                    response.tool_calls.len() as u32,
                    response.usage,
                )
            }
        };

    // Step 4: Extract memories (unchanged — operates on user/assistant text).
    let extraction_messages = build_extraction_window(&messages, user_input, &final_content);
    let agent_id_owned = *agent_id;
    let extractor = runtime.memory_extractor.clone();
    let memory_svc = runtime.memory.clone();
    tokio::spawn(async move {
        match extractor
            .extract(&agent_id_owned, &extraction_messages)
            .await
        {
            Ok(candidates) if !candidates.is_empty() => {
                for candidate in candidates {
                    let entry = MemoryEntry::from_candidate(candidate, Some(agent_id_owned));
                    if let Err(e) = memory_svc.store(entry).await {
                        warn!("failed to store extracted memory: {}", e);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => warn!("memory extraction failed: {}", e),
        }
    });

    info!(%agent_id, "agent loop: turn complete");
    Ok(AgentResponse {
        content: final_content,
        tool_calls_made,
        tokens_used: usage,
    })
}

/// Handle a multi-step request: create a Task + Run for the autonomy system.
///
/// Returns an AgentResponse summarizing that a task has been created and
/// is queued for execution by the TaskExecutor.
async fn handle_multi_step(
    _runtime: &Runtime,
    agent_id: &AgentId,
    user_input: &str,
    task_registry: std::sync::Arc<dyn clawx_types::traits::TaskRegistryPort>,
) -> Result<AgentResponse> {
    use chrono::Utc;

    let now = Utc::now();
    let task = Task {
        id: TaskId::new(),
        agent_id: *agent_id,
        name: truncate_for_name(user_input),
        goal: user_input.to_string(),
        source_kind: TaskSourceKind::Conversation,
        lifecycle_status: TaskLifecycleStatus::Active,
        default_max_steps: 10,
        default_timeout_secs: 300,
        notification_policy: serde_json::json!({}),
        suppression_state: SuppressionState::default(),
        last_run_at: None,
        next_run_at: None,
        created_at: now,
        updated_at: now,
    };

    let task_id = task_registry.create_task(task).await?;

    let run = Run {
        id: RunId::new(),
        task_id,
        trigger_id: None,
        idempotency_key: format!("conversation:{}:{}", agent_id, now.timestamp_millis()),
        run_status: RunStatus::Queued,
        attempt: 1,
        lease_owner: None,
        lease_expires_at: None,
        checkpoint: serde_json::json!({}),
        tokens_used: 0,
        steps_count: 0,
        result_summary: None,
        failure_reason: None,
        feedback_kind: None,
        feedback_reason: None,
        notification_status: NotificationStatus::default(),
        triggered_at: now,
        started_at: None,
        finished_at: None,
        created_at: now,
    };

    let run_id = task_registry.create_run(run).await?;

    info!(%agent_id, %task_id, %run_id, "created task + run for multi-step execution");

    // Return a response indicating the task is being processed.
    // The TaskExecutor will pick up the queued Run asynchronously.
    let content = format!(
        "I've identified this as a multi-step task and created an execution plan.\n\
         Task: {}\nRun: {}\nStatus: Queued for execution.",
        task_id, run_id
    );

    Ok(AgentResponse {
        content,
        tool_calls_made: 0,
        tokens_used: TokenUsage::default(),
    })
}

/// Truncate user input to a reasonable task name (max 80 chars).
/// Uses char count to avoid panic on multi-byte UTF-8 input.
fn truncate_for_name(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.chars().count() <= 80 {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(77).collect();
        format!("{}...", truncated)
    }
}

/// Build the message window for memory extraction (last user + assistant pair).
fn build_extraction_window(
    _context_messages: &[Message],
    user_input: &str,
    assistant_response: &str,
) -> Vec<Message> {
    vec![
        Message {
            role: MessageRole::User,
            content: user_input.to_string(),
            blocks: vec![],
            tool_call_id: None,
        },
        Message {
            role: MessageRole::Assistant,
            content: assistant_response.to_string(),
            blocks: vec![],
            tool_call_id: None,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    async fn make_runtime() -> Runtime {
        Runtime::new(
            crate::db::Database::in_memory().await.unwrap(),
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
    async fn run_turn_returns_stub_response() {
        let rt = make_runtime().await;
        let agent_id = AgentId::new();
        let conversation = Conversation {
            id: clawx_types::ids::ConversationId::new(),
            agent_id: agent_id.clone(),
            title: None,
            status: ConversationStatus::Active,
            messages: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };

        let resp = run_turn(&rt, &agent_id, &conversation, "Hello")
            .await
            .unwrap();
        assert!(resp.content.contains("[stub]"));
        assert_eq!(resp.tool_calls_made, 0);
    }
}
