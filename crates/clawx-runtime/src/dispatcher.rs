//! Task dispatcher — routes user messages to the appropriate agent loop.
//!
//! In Phase 3 this is a simple pass-through. Future phases will add
//! queueing, concurrency limits, and priority scheduling.

use clawx_types::agent::{AgentResponse, Conversation, UserMessage};
use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use tracing::info;

use crate::Runtime;

/// Dispatch a user message to the agent loop and return the response.
pub async fn dispatch(
    runtime: &Runtime,
    agent_id: &AgentId,
    conversation: &Conversation,
    message: &UserMessage,
) -> Result<AgentResponse> {
    info!(%agent_id, "dispatching message to agent");
    crate::agent_loop::run_turn(runtime, agent_id, conversation, &message.content).await
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
    async fn dispatch_returns_response() {
        let rt = make_runtime().await;
        let agent_id = AgentId::new();
        let conversation = Conversation {
            id: clawx_types::ids::ConversationId::new(),
            agent_id: agent_id.clone(),
            title: None,
            status: clawx_types::agent::ConversationStatus::Active,
            messages: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let msg = UserMessage {
            content: "test".to_string(),
            attachments: vec![],
        };

        let resp = dispatch(&rt, &agent_id, &conversation, &msg).await.unwrap();
        assert!(resp.content.contains("[stub]"));
    }
}
