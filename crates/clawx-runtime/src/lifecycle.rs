//! Agent lifecycle state machine: Idle → Active → Error → Offline.

use std::collections::HashMap;
use std::sync::Arc;

use clawx_types::agent::AgentStatus;
use clawx_types::ids::AgentId;
use tokio::sync::RwLock;
use tracing::info;

/// Manages the lifecycle state of all agents.
#[derive(Debug, Clone)]
pub struct LifecycleManager {
    states: Arc<RwLock<HashMap<AgentId, AgentStatus>>>,
}

impl LifecycleManager {
    pub fn new() -> Self {
        Self {
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn get_status(&self, agent_id: &AgentId) -> AgentStatus {
        self.states
            .read()
            .await
            .get(agent_id)
            .copied()
            .unwrap_or(AgentStatus::Offline)
    }

    pub async fn set_status(&self, agent_id: AgentId, status: AgentStatus) {
        info!(%agent_id, ?status, "agent status changed");
        self.states.write().await.insert(agent_id, status);
    }

    pub async fn activate(&self, agent_id: AgentId) {
        self.set_status(agent_id, AgentStatus::Active).await;
    }

    pub async fn deactivate(&self, agent_id: AgentId) {
        self.set_status(agent_id, AgentStatus::Idle).await;
    }

    pub async fn mark_error(&self, agent_id: AgentId) {
        self.set_status(agent_id, AgentStatus::Error).await;
    }

    pub async fn remove(&self, agent_id: &AgentId) {
        self.states.write().await.remove(agent_id);
    }

    pub async fn active_count(&self) -> usize {
        self.states
            .read()
            .await
            .values()
            .filter(|s| **s == AgentStatus::Active)
            .count()
    }
}

impl Default for LifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn default_status_is_offline() {
        let mgr = LifecycleManager::new();
        let id = AgentId::new();
        assert_eq!(mgr.get_status(&id).await, AgentStatus::Offline);
    }

    #[tokio::test]
    async fn activate_sets_active() {
        let mgr = LifecycleManager::new();
        let id = AgentId::new();
        mgr.activate(id.clone()).await;
        assert_eq!(mgr.get_status(&id).await, AgentStatus::Active);
    }

    #[tokio::test]
    async fn deactivate_sets_idle() {
        let mgr = LifecycleManager::new();
        let id = AgentId::new();
        mgr.activate(id.clone()).await;
        mgr.deactivate(id.clone()).await;
        assert_eq!(mgr.get_status(&id).await, AgentStatus::Idle);
    }

    #[tokio::test]
    async fn active_count_tracks_active_agents() {
        let mgr = LifecycleManager::new();
        let id1 = AgentId::new();
        let id2 = AgentId::new();
        mgr.activate(id1.clone()).await;
        mgr.activate(id2.clone()).await;
        assert_eq!(mgr.active_count().await, 2);
        mgr.deactivate(id1).await;
        assert_eq!(mgr.active_count().await, 1);
    }

    #[tokio::test]
    async fn remove_clears_state() {
        let mgr = LifecycleManager::new();
        let id = AgentId::new();
        mgr.activate(id.clone()).await;
        mgr.remove(&id).await;
        assert_eq!(mgr.get_status(&id).await, AgentStatus::Offline);
    }
}
