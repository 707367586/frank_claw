//! Stub memory service implementations.

use async_trait::async_trait;
use clawx_types::agent::Conversation;
use clawx_types::error::Result;
use clawx_types::ids::*;
use clawx_types::memory::*;
use clawx_types::pagination::*;
use clawx_types::traits::{MemoryService, WorkingMemoryManager};

/// Stub memory service — stores nothing, returns empty results.
#[derive(Debug, Clone)]
pub struct StubMemoryService;

#[async_trait]
impl MemoryService for StubMemoryService {
    async fn store(&self, entry: MemoryEntry) -> Result<MemoryId> {
        Ok(entry.id)
    }

    async fn recall(&self, _query: MemoryQuery) -> Result<Vec<ScoredMemory>> {
        Ok(vec![])
    }

    async fn update(&self, _update: MemoryUpdate) -> Result<()> {
        Ok(())
    }

    async fn delete(&self, _id: MemoryId) -> Result<()> {
        Ok(())
    }

    async fn toggle_pin(&self, _id: MemoryId, _pinned: bool) -> Result<()> {
        Ok(())
    }

    async fn get(&self, _id: MemoryId) -> Result<Option<MemoryEntry>> {
        Ok(None)
    }

    async fn list(
        &self,
        _filter: MemoryFilter,
        pagination: Pagination,
    ) -> Result<PagedResult<MemoryEntry>> {
        Ok(PagedResult {
            items: vec![],
            total: 0,
            page: pagination.page,
            per_page: pagination.per_page,
        })
    }

    async fn stats(&self, _agent_id: Option<AgentId>) -> Result<MemoryStats> {
        Ok(MemoryStats {
            total_count: 0,
            agent_count: 0,
            user_count: 0,
            pinned_count: 0,
            archived_count: 0,
        })
    }
}

/// Stub working memory manager — returns minimal assembled context.
#[derive(Debug, Clone)]
pub struct StubWorkingMemoryManager;

#[async_trait]
impl WorkingMemoryManager for StubWorkingMemoryManager {
    async fn assemble_context(
        &self,
        _agent_id: &AgentId,
        conversation: &Conversation,
        _user_input: &str,
    ) -> Result<AssembledContext> {
        let history = conversation
            .messages
            .iter()
            .map(|m| clawx_types::llm::Message {
                role: m.role,
                content: m.content.clone(),
                blocks: vec![],
                tool_call_id: None,
            })
            .collect();

        Ok(AssembledContext {
            system_prompt: "You are a helpful assistant.".to_string(),
            recalled_memories: vec![],
            knowledge_snippets: vec![],
            conversation_history: history,
            total_tokens: 0,
            memory_tokens: 0,
        })
    }

    async fn compress_if_needed(
        &self,
        _agent_id: &AgentId,
        _conversation: &mut Conversation,
    ) -> Result<bool> {
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn stub_memory_store_returns_id() {
        let svc = StubMemoryService;
        let id = MemoryId::new();
        let entry = MemoryEntry {
            id: id.clone(),
            scope: MemoryScope::Agent,
            agent_id: Some(AgentId::new()),
            kind: MemoryKind::Fact,
            summary: "test".to_string(),
            content: serde_json::json!({}),
            importance: 5.0,
            freshness: 1.0,
            access_count: 0,
            is_pinned: false,
            source_agent_id: None,
            source_type: SourceType::Implicit,
            superseded_by: None,
            qdrant_point_id: None,
            created_at: chrono::Utc::now(),
            last_accessed_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let result = svc.store(entry).await.unwrap();
        assert_eq!(result, id);
    }

    #[tokio::test]
    async fn stub_memory_recall_empty() {
        let svc = StubMemoryService;
        let query = MemoryQuery {
            query_text: Some("test".to_string()),
            scope: None,
            agent_id: None,
            top_k: 5,
            include_archived: false,
            token_budget: None,
        };
        let results = svc.recall(query).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn stub_memory_stats_zero() {
        let svc = StubMemoryService;
        let stats = svc.stats(None).await.unwrap();
        assert_eq!(stats.total_count, 0);
    }

    #[tokio::test]
    async fn stub_working_memory_assembles_context() {
        let mgr = StubWorkingMemoryManager;
        let conversation = Conversation {
            id: clawx_types::ids::ConversationId::new(),
            agent_id: AgentId::new(),
            title: None,
            status: clawx_types::agent::ConversationStatus::Active,
            messages: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        let ctx = mgr
            .assemble_context(&AgentId::new(), &conversation, "hello")
            .await
            .unwrap();
        assert!(!ctx.system_prompt.is_empty());
        assert!(ctx.recalled_memories.is_empty());
    }

    #[test]
    fn memory_traits_are_object_safe() {
        fn _assert_mem(_: Arc<dyn MemoryService>) {}
        fn _assert_wm(_: Arc<dyn WorkingMemoryManager>) {}
    }
}
