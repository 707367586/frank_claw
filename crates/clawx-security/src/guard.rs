//! Security guard implementations.
//!
//! - `PermissiveSecurityGuard` — allows everything (dev/testing).
//! - `ClawxSecurityGuard` — real DLP + capability + path checks.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use clawx_types::security::*;
use clawx_types::traits::SecurityService;

use crate::dlp::DlpScanner;
use crate::network::NetworkChecker;
use crate::sandbox::{check_path_safety, CapabilityChecker};

/// A security guard that allows all operations.
/// Used as a default during skeleton phase.
#[derive(Debug, Clone)]
pub struct PermissiveSecurityGuard;

#[async_trait]
impl SecurityService for PermissiveSecurityGuard {
    async fn check_capability(
        &self,
        _agent_id: &AgentId,
        _capability: Capability,
    ) -> Result<SecurityDecision> {
        Ok(SecurityDecision::Allow)
    }

    async fn scan_dlp(
        &self,
        _content: &str,
        direction: DataDirection,
    ) -> Result<DlpResult> {
        Ok(DlpResult {
            passed: true,
            direction,
            violations: vec![],
            redacted_content: None,
        })
    }

    async fn check_network(&self, _url: &str) -> Result<SecurityDecision> {
        Ok(SecurityDecision::Allow)
    }

    async fn check_path(&self, _path: &str) -> Result<SecurityDecision> {
        Ok(SecurityDecision::Allow)
    }
}

// ---------------------------------------------------------------------------
// ClawxSecurityGuard — real implementation with DLP, capabilities, and paths.
// ---------------------------------------------------------------------------

/// Production security guard integrating DLP, capability, path, and network checks.
pub struct ClawxSecurityGuard {
    dlp: DlpScanner,
    /// Per-agent capability lists. Key = agent_id string, Value = list of granted capabilities.
    agent_capabilities: RwLock<HashMap<String, Vec<String>>>,
    /// Allowed directories for path checks.
    allowed_dirs: Vec<String>,
    /// Network whitelist + SSRF checker.
    network: NetworkChecker,
}

impl ClawxSecurityGuard {
    /// Create a new ClawxSecurityGuard with default DLP patterns.
    pub fn new(allowed_dirs: Vec<String>) -> Self {
        Self {
            dlp: DlpScanner::default_patterns(),
            agent_capabilities: RwLock::new(HashMap::new()),
            allowed_dirs,
            network: NetworkChecker::new(vec![], true),
        }
    }

    /// Create a new ClawxSecurityGuard with network whitelist.
    pub fn with_network_whitelist(
        allowed_dirs: Vec<String>,
        allowed_domains: Vec<String>,
    ) -> Self {
        Self {
            dlp: DlpScanner::default_patterns(),
            agent_capabilities: RwLock::new(HashMap::new()),
            allowed_dirs,
            network: NetworkChecker::new(allowed_domains, true),
        }
    }

    /// Register capabilities for an agent.
    pub fn register_agent_capabilities(&self, agent_id: &AgentId, capabilities: Vec<String>) {
        let mut caps = self.agent_capabilities.write().unwrap();
        caps.insert(agent_id.to_string(), capabilities);
    }
}

impl std::fmt::Debug for ClawxSecurityGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClawxSecurityGuard")
            .field("allowed_dirs", &self.allowed_dirs)
            .finish()
    }
}

#[async_trait]
impl SecurityService for ClawxSecurityGuard {
    async fn check_capability(
        &self,
        agent_id: &AgentId,
        capability: Capability,
    ) -> Result<SecurityDecision> {
        let caps = self.agent_capabilities.read().unwrap();
        let agent_caps = caps.get(&agent_id.to_string());
        match agent_caps {
            Some(granted) => Ok(CapabilityChecker::check(granted, capability)),
            None => Ok(SecurityDecision::Deny {
                reason: format!("no capabilities registered for agent {}", agent_id),
            }),
        }
    }

    async fn scan_dlp(
        &self,
        content: &str,
        direction: DataDirection,
    ) -> Result<DlpResult> {
        Ok(self.dlp.scan(content, direction))
    }

    async fn check_network(&self, url: &str) -> Result<SecurityDecision> {
        Ok(self.network.check_url(url))
    }

    async fn check_path(&self, path: &str) -> Result<SecurityDecision> {
        Ok(check_path_safety(path, &self.allowed_dirs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // -------------------------------------------------------------------
    // PermissiveSecurityGuard tests
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn permissive_guard_allows_capability() {
        let guard = PermissiveSecurityGuard;
        let agent_id = AgentId::new();
        let decision = guard
            .check_capability(&agent_id, Capability::FsRead)
            .await
            .unwrap();
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[tokio::test]
    async fn permissive_guard_dlp_clean() {
        let guard = PermissiveSecurityGuard;
        let result = guard
            .scan_dlp("some secret content", DataDirection::Outbound)
            .await
            .unwrap();
        assert!(result.passed);
        assert!(result.violations.is_empty());
    }

    #[tokio::test]
    async fn permissive_guard_allows_network() {
        let guard = PermissiveSecurityGuard;
        let decision = guard.check_network("https://example.com").await.unwrap();
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[tokio::test]
    async fn permissive_guard_allows_path() {
        let guard = PermissiveSecurityGuard;
        let decision = guard.check_path("/etc/passwd").await.unwrap();
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[test]
    fn security_service_is_object_safe() {
        fn _assert(_: Arc<dyn SecurityService>) {}
    }

    // -------------------------------------------------------------------
    // ClawxSecurityGuard tests
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn clawx_guard_dlp_blocks_ssh_key() {
        let guard = ClawxSecurityGuard::new(vec![]);
        let result = guard
            .scan_dlp(
                "-----BEGIN RSA PRIVATE KEY-----\ndata",
                DataDirection::Outbound,
            )
            .await
            .unwrap();
        assert!(!result.passed);
        assert!(!result.violations.is_empty());
    }

    #[tokio::test]
    async fn clawx_guard_dlp_passes_clean() {
        let guard = ClawxSecurityGuard::new(vec![]);
        let result = guard
            .scan_dlp("Hello world", DataDirection::Outbound)
            .await
            .unwrap();
        assert!(result.passed);
    }

    #[tokio::test]
    async fn clawx_guard_capability_allow() {
        let guard = ClawxSecurityGuard::new(vec![]);
        let agent_id = AgentId::new();
        guard.register_agent_capabilities(&agent_id, vec!["fs_read".to_string()]);

        let decision = guard
            .check_capability(&agent_id, Capability::FsRead)
            .await
            .unwrap();
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[tokio::test]
    async fn clawx_guard_capability_deny() {
        let guard = ClawxSecurityGuard::new(vec![]);
        let agent_id = AgentId::new();
        guard.register_agent_capabilities(&agent_id, vec!["fs_read".to_string()]);

        let decision = guard
            .check_capability(&agent_id, Capability::ExecShell)
            .await
            .unwrap();
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn clawx_guard_capability_unregistered_agent() {
        let guard = ClawxSecurityGuard::new(vec![]);
        let agent_id = AgentId::new();

        let decision = guard
            .check_capability(&agent_id, Capability::FsRead)
            .await
            .unwrap();
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn clawx_guard_path_allows_within_dir() {
        let guard = ClawxSecurityGuard::new(vec!["/home/user".to_string()]);
        let decision = guard.check_path("/home/user/file.txt").await.unwrap();
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[tokio::test]
    async fn clawx_guard_path_denies_traversal() {
        let guard = ClawxSecurityGuard::new(vec!["/home/user".to_string()]);
        let decision = guard.check_path("/home/user/../etc/passwd").await.unwrap();
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn clawx_guard_path_denies_outside_dir() {
        let guard = ClawxSecurityGuard::new(vec!["/home/user".to_string()]);
        let decision = guard.check_path("/etc/passwd").await.unwrap();
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn clawx_guard_network_blocks_private_ip() {
        let guard = ClawxSecurityGuard::new(vec![]);
        let decision = guard.check_network("http://127.0.0.1/secret").await.unwrap();
        assert!(matches!(decision, SecurityDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn clawx_guard_network_allows_public() {
        let guard = ClawxSecurityGuard::new(vec![]);
        let decision = guard.check_network("https://api.example.com/v1").await.unwrap();
        assert_eq!(decision, SecurityDecision::Allow);
    }

    #[tokio::test]
    async fn clawx_guard_network_whitelist_restricts() {
        let guard = ClawxSecurityGuard::with_network_whitelist(
            vec![],
            vec!["api.openai.com".to_string()],
        );
        assert_eq!(
            guard.check_network("https://api.openai.com/v1").await.unwrap(),
            SecurityDecision::Allow,
        );
        assert!(matches!(
            guard.check_network("https://evil.com/steal").await.unwrap(),
            SecurityDecision::Deny { .. }
        ));
    }

    #[test]
    fn clawx_guard_is_object_safe() {
        fn _assert(_: Arc<dyn SecurityService>) {}
    }
}
