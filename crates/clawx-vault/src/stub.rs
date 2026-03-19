//! Stub vault service that returns empty data.

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::ids::*;
use clawx_types::traits::VaultService;
use clawx_types::vault::*;

/// A stub vault service. Returns empty snapshots and no-ops on rollback.
#[derive(Debug, Clone)]
pub struct StubVaultService;

impl StubVaultService {
    fn make_snapshot(
        agent_id: Option<AgentId>,
        task_id: Option<TaskId>,
        description: Option<String>,
    ) -> VaultSnapshot {
        VaultSnapshot {
            id: SnapshotId::new(),
            label: "stub-snapshot".to_string(),
            agent_id,
            task_id,
            description,
            disk_size: 0,
            created_at: chrono::Utc::now(),
        }
    }
}

#[async_trait]
impl VaultService for StubVaultService {
    async fn create_snapshot(
        &self,
        agent_id: Option<AgentId>,
        task_id: Option<TaskId>,
        description: Option<String>,
    ) -> Result<VaultSnapshot> {
        Ok(Self::make_snapshot(agent_id, task_id, description))
    }

    async fn list_snapshots(&self) -> Result<Vec<VaultSnapshot>> {
        Ok(vec![])
    }

    async fn diff_preview(&self, _snapshot_id: SnapshotId) -> Result<DiffPreview> {
        Ok(DiffPreview {
            snapshot: Self::make_snapshot(None, None, None),
            changes: vec![],
        })
    }

    async fn rollback(&self, _snapshot_id: SnapshotId) -> Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn stub_create_snapshot() {
        let svc = StubVaultService;
        let snap = svc
            .create_snapshot(None, None, Some("test".into()))
            .await
            .unwrap();
        assert_eq!(snap.description, Some("test".to_string()));
        assert_eq!(snap.disk_size, 0);
    }

    #[tokio::test]
    async fn stub_list_empty() {
        let svc = StubVaultService;
        assert!(svc.list_snapshots().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn stub_diff_preview_empty() {
        let svc = StubVaultService;
        let preview = svc.diff_preview(SnapshotId::new()).await.unwrap();
        assert!(preview.changes.is_empty());
    }

    #[tokio::test]
    async fn stub_rollback_noop() {
        StubVaultService.rollback(SnapshotId::new()).await.unwrap();
    }

    #[test]
    fn vault_service_is_object_safe() {
        fn _assert(_: Arc<dyn VaultService>) {}
    }
}
