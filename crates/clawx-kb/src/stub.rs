//! Stub knowledge service implementation.

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::ids::*;
use clawx_types::knowledge::*;
use clawx_types::traits::KnowledgeService;

/// A stub knowledge service that returns empty results.
#[derive(Debug, Clone)]
pub struct StubKnowledgeService;

#[async_trait]
impl KnowledgeService for StubKnowledgeService {
    async fn search(&self, _query: SearchQuery) -> Result<Vec<SearchResult>> {
        Ok(vec![])
    }

    async fn add_source(
        &self,
        _path: String,
        _agent_id: Option<AgentId>,
    ) -> Result<KnowledgeSourceId> {
        Ok(KnowledgeSourceId::new())
    }

    async fn remove_source(&self, _source_id: KnowledgeSourceId) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn stub_search_returns_empty() {
        let svc = StubKnowledgeService;
        let query = SearchQuery {
            query_text: "test".to_string(),
            agent_id: None,
            top_n: 5,
        };
        let results = svc.search(query).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn stub_add_source_returns_id() {
        let svc = StubKnowledgeService;
        let id = svc.add_source("/tmp/test.md".into(), None).await.unwrap();
        // Just verify it doesn't panic and returns a valid ID
        let _ = id.to_string();
    }

    #[tokio::test]
    async fn stub_remove_source_noop() {
        let svc = StubKnowledgeService;
        svc.remove_source(KnowledgeSourceId::new()).await.unwrap();
    }

    #[test]
    fn knowledge_service_is_object_safe() {
        fn _assert(_: Arc<dyn KnowledgeService>) {}
    }
}
