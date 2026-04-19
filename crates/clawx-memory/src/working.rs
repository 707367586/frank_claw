//! Real WorkingMemoryManager implementation.
//!
//! Assembles the context window for each agent turn:
//! 1. System prompt (from agent config or default)
//! 2. Recalled long-term memories relevant to the conversation
//! 3. Conversation history (trimmed to fit token budget)

use std::sync::Arc;

use async_trait::async_trait;
use tracing::{debug, warn};

use clawx_types::agent::Conversation;
use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use clawx_types::llm::Message;
use clawx_types::memory::*;
use clawx_types::traits::{MemoryService, WorkingMemoryManager};

/// Approximate token count for a string (rough: 1 token ≈ 4 chars).
fn estimate_tokens(text: &str) -> u32 {
    (text.len() as u32).div_ceil(4)
}

/// Configuration for context assembly.
#[derive(Debug, Clone)]
pub struct WorkingMemoryConfig {
    /// Maximum total tokens for the assembled context.
    pub max_context_tokens: u32,
    /// Maximum tokens reserved for recalled memories.
    pub max_memory_tokens: u32,
    /// Number of top memories to recall.
    pub recall_top_k: usize,
    /// Default system prompt when agent has none configured.
    pub default_system_prompt: String,
}

impl Default for WorkingMemoryConfig {
    fn default() -> Self {
        Self {
            max_context_tokens: 8192,
            max_memory_tokens: 1024,
            recall_top_k: 5,
            default_system_prompt: "You are a helpful AI assistant.".to_string(),
        }
    }
}

/// Real working memory manager that recalls memories and assembles context.
pub struct RealWorkingMemoryManager {
    memory: Arc<dyn MemoryService>,
    config: WorkingMemoryConfig,
}

impl RealWorkingMemoryManager {
    pub fn new(memory: Arc<dyn MemoryService>, config: WorkingMemoryConfig) -> Self {
        Self { memory, config }
    }
}

impl std::fmt::Debug for RealWorkingMemoryManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealWorkingMemoryManager")
            .field("config", &self.config)
            .finish()
    }
}

#[async_trait]
impl WorkingMemoryManager for RealWorkingMemoryManager {
    async fn assemble_context(
        &self,
        agent_id: &AgentId,
        conversation: &Conversation,
        user_input: &str,
    ) -> Result<AssembledContext> {
        let system_prompt = self.config.default_system_prompt.clone();
        let mut total_tokens = estimate_tokens(&system_prompt);

        // 1. Recall relevant memories
        let recalled = self
            .memory
            .recall(MemoryQuery {
                query_text: Some(user_input.to_string()),
                scope: None,
                agent_id: Some(*agent_id),
                top_k: self.config.recall_top_k,
                include_archived: false,
                token_budget: Some(self.config.max_memory_tokens),
            })
            .await
            .unwrap_or_else(|e| {
                warn!("memory recall failed, continuing without memories: {}", e);
                vec![]
            });

        // Calculate memory token usage
        let memory_tokens: u32 = recalled
            .iter()
            .map(|m| estimate_tokens(&m.entry.summary) + estimate_tokens(&m.entry.content.to_string()))
            .sum();
        total_tokens += memory_tokens;

        // 2. Build conversation history (newest messages first, then reverse)
        let mut history: Vec<Message> = Vec::new();
        let remaining_budget = self
            .config
            .max_context_tokens
            .saturating_sub(total_tokens)
            .saturating_sub(estimate_tokens(user_input));

        let mut history_tokens = 0u32;
        for msg in conversation.messages.iter().rev() {
            let msg_tokens = estimate_tokens(&msg.content);
            if history_tokens + msg_tokens > remaining_budget {
                debug!(
                    dropped = conversation.messages.len() - history.len(),
                    "trimmed conversation history to fit token budget"
                );
                break;
            }
            history_tokens += msg_tokens;
            history.push(Message {
                role: msg.role,
                content: msg.content.clone(),
                blocks: vec![],
                tool_call_id: None,
            });
        }
        history.reverse();
        total_tokens += history_tokens;

        // 3. Add user input tokens
        total_tokens += estimate_tokens(user_input);

        Ok(AssembledContext {
            system_prompt,
            recalled_memories: recalled,
            knowledge_snippets: vec![],
            conversation_history: history,
            total_tokens,
            memory_tokens,
        })
    }

    async fn compress_if_needed(
        &self,
        _agent_id: &AgentId,
        _conversation: &mut Conversation,
    ) -> Result<bool> {
        // Compression not implemented in v0.1 — history trimming in assemble_context
        // handles the token budget already
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StubMemoryService;
    use clawx_types::agent::ConversationStatus;
    use clawx_types::ids::ConversationId;
    use clawx_types::llm::MessageRole;

    fn make_conversation(message_count: usize) -> Conversation {
        let agent_id = AgentId::new();
        let messages = (0..message_count)
            .map(|i| clawx_types::agent::ConversationMessage {
                id: clawx_types::ids::MessageId::new(),
                conversation_id: ConversationId::new(),
                role: if i % 2 == 0 {
                    MessageRole::User
                } else {
                    MessageRole::Assistant
                },
                content: format!("Message number {}", i),
                metadata: None,
                created_at: chrono::Utc::now(),
            })
            .collect();

        Conversation {
            id: ConversationId::new(),
            agent_id,
            title: None,
            status: ConversationStatus::Active,
            messages,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn estimate_tokens_basic() {
        assert_eq!(estimate_tokens(""), 0); // (0+3)/4 = 0
        assert_eq!(estimate_tokens("hi"), 1); // (2+3)/4 = 1
        assert_eq!(estimate_tokens("hello world"), 3); // (11+3)/4 = 3
    }

    #[tokio::test]
    async fn assembles_context_with_empty_conversation() {
        let memory = Arc::new(StubMemoryService);
        let mgr = RealWorkingMemoryManager::new(memory, WorkingMemoryConfig::default());
        let conv = make_conversation(0);
        let agent_id = conv.agent_id;

        let ctx = mgr
            .assemble_context(&agent_id, &conv, "hello")
            .await
            .unwrap();

        assert!(!ctx.system_prompt.is_empty());
        assert!(ctx.recalled_memories.is_empty());
        assert!(ctx.conversation_history.is_empty());
        assert!(ctx.total_tokens > 0);
    }

    #[tokio::test]
    async fn assembles_context_with_conversation_history() {
        let memory = Arc::new(StubMemoryService);
        let mgr = RealWorkingMemoryManager::new(memory, WorkingMemoryConfig::default());
        let conv = make_conversation(4);
        let agent_id = conv.agent_id;

        let ctx = mgr
            .assemble_context(&agent_id, &conv, "what did I say?")
            .await
            .unwrap();

        assert_eq!(ctx.conversation_history.len(), 4);
        // First message should be the oldest (User)
        assert_eq!(ctx.conversation_history[0].role, MessageRole::User);
    }

    #[tokio::test]
    async fn trims_history_to_fit_token_budget() {
        let memory = Arc::new(StubMemoryService);
        let config = WorkingMemoryConfig {
            max_context_tokens: 50, // very small budget
            ..Default::default()
        };
        let mgr = RealWorkingMemoryManager::new(memory, config);
        // Create many messages to exceed budget
        let conv = make_conversation(100);
        let agent_id = conv.agent_id;

        let ctx = mgr
            .assemble_context(&agent_id, &conv, "test")
            .await
            .unwrap();

        // Should have trimmed history
        assert!(ctx.conversation_history.len() < 100);
        assert!(ctx.total_tokens <= 50 + 20); // some slack for estimates
    }

    #[tokio::test]
    async fn compress_if_needed_returns_false() {
        let memory = Arc::new(StubMemoryService);
        let mgr = RealWorkingMemoryManager::new(memory, WorkingMemoryConfig::default());
        let mut conv = make_conversation(0);
        let agent_id = conv.agent_id;

        let compressed = mgr.compress_if_needed(&agent_id, &mut conv).await.unwrap();
        assert!(!compressed);
    }

    #[test]
    fn working_memory_manager_is_object_safe() {
        fn _assert(_: Arc<dyn WorkingMemoryManager>) {}
    }
}
