//! The core agent loop: receive message → assemble context → call LLM → return response.

use clawx_types::agent::*;
use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use clawx_types::llm::*;
use clawx_types::memory::*;
use tracing::{debug, info, warn};

use crate::Runtime;

/// Run a single turn of the agent loop.
///
/// 1. Assemble context (system prompt + memories + conversation history)
/// 2. Call LLM
/// 3. Extract memories from the conversation
/// 4. Return the response
pub async fn run_turn(
    runtime: &Runtime,
    agent_id: &AgentId,
    conversation: &Conversation,
    user_input: &str,
) -> Result<AgentResponse> {
    info!(%agent_id, "agent loop: running turn");

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

    // Step 2: Build completion request — inject recalled memories into system prompt
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
            tool_call_id: None,
        });
    }

    // Conversation history
    messages.extend(ctx.conversation_history);

    // Current user message
    messages.push(Message {
        role: MessageRole::User,
        content: user_input.to_string(),
        tool_call_id: None,
    });

    let request = CompletionRequest {
        model: "default".to_string(),
        messages: messages.clone(),
        tools: None,
        temperature: None,
        max_tokens: Some(4096),
        stream: false,
    };

    // Step 3: Call LLM
    let response = runtime.llm.complete(request).await?;

    // Step 4: Extract memories from the last few messages (background, best-effort)
    let extraction_messages = build_extraction_window(&messages, user_input, &response.content);
    let agent_id_owned = *agent_id;
    let extractor = runtime.memory_extractor.clone();
    let memory_svc = runtime.memory.clone();

    tokio::spawn(async move {
        match extractor.extract(&agent_id_owned, &extraction_messages).await {
            Ok(candidates) if !candidates.is_empty() => {
                debug!(count = candidates.len(), "extracted memory candidates");
                for candidate in candidates {
                    let entry = MemoryEntry::from_candidate(candidate, Some(agent_id_owned));
                    if let Err(e) = memory_svc.store(entry).await {
                        warn!("failed to store extracted memory: {}", e);
                    }
                }
            }
            Ok(_) => {} // no candidates
            Err(e) => {
                warn!("memory extraction failed: {}", e);
            }
        }
    });

    info!(%agent_id, "agent loop: turn complete");

    Ok(AgentResponse {
        content: response.content,
        tool_calls_made: response.tool_calls.len() as u32,
        tokens_used: response.usage,
    })
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
            tool_call_id: None,
        },
        Message {
            role: MessageRole::Assistant,
            content: assistant_response.to_string(),
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

        let resp = run_turn(&rt, &agent_id, &conversation, "Hello").await.unwrap();
        assert!(resp.content.contains("[stub]"));
        assert_eq!(resp.tool_calls_made, 0);
    }
}
