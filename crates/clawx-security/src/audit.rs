//! SHA-256 hash-chain audit log.
//!
//! Each audit entry contains a hash of the previous entry, forming a
//! tamper-evident chain.

use chrono::Utc;
use std::collections::VecDeque;
use std::sync::Mutex;

use clawx_types::ids::{AgentId, AuditEntryId};
use clawx_types::security::{AuditEntry, SecurityDecision};

/// Compute a SHA-256 hash hex digest of the given bytes.
fn hash_hex(data: &[u8]) -> String {
    use sha2::{Sha256, Digest};
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// In-memory audit log with hash-chain integrity.
pub struct AuditLog {
    entries: Mutex<VecDeque<AuditEntry>>,
    max_entries: usize,
}

impl AuditLog {
    /// Create a new audit log with the given maximum capacity.
    pub fn new(max_entries: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(max_entries)),
            max_entries,
        }
    }

    /// Append an audit entry to the log.
    ///
    /// Computes the hash chain automatically.
    pub fn append(
        &self,
        agent_id: AgentId,
        action: String,
        decision: SecurityDecision,
        details: Option<serde_json::Value>,
    ) -> AuditEntry {
        let mut entries = self.entries.lock().unwrap();

        let prev_hash = entries
            .back()
            .map(|e| e.hash.clone())
            .unwrap_or_else(|| "0".repeat(64));

        let id = AuditEntryId::new();
        let timestamp = Utc::now();

        // Build the data to hash: prev_hash + id + timestamp + action + decision
        let hash_input = format!(
            "{}|{}|{}|{}|{:?}",
            prev_hash, id, timestamp, action, decision
        );
        let hash = hash_hex(hash_input.as_bytes());

        let entry = AuditEntry {
            id,
            timestamp,
            agent_id,
            action,
            decision,
            details,
            prev_hash,
            hash,
        };

        if entries.len() >= self.max_entries {
            entries.pop_front();
        }
        entries.push_back(entry.clone());

        entry
    }

    /// Get all entries in the log.
    pub fn entries(&self) -> Vec<AuditEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }

    /// Verify the hash chain integrity.
    ///
    /// Returns `true` if the chain is valid.
    pub fn verify_chain(&self) -> bool {
        let entries = self.entries.lock().unwrap();

        for i in 1..entries.len() {
            if entries[i].prev_hash != entries[i - 1].hash {
                return false;
            }
        }

        // Verify first entry's prev_hash is all zeros
        if let Some(first) = entries.front() {
            if first.prev_hash != "0".repeat(64) {
                return false;
            }
        }

        true
    }

    /// Number of entries in the log.
    pub fn len(&self) -> usize {
        self.entries.lock().unwrap().len()
    }

    /// Whether the log is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.lock().unwrap().is_empty()
    }
}

impl std::fmt::Debug for AuditLog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.entries.lock().unwrap().len();
        f.debug_struct("AuditLog")
            .field("entries_count", &len)
            .field("max_entries", &self.max_entries)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::ids::AgentId;
    use clawx_types::security::SecurityDecision;

    #[test]
    fn append_creates_entry_with_hash() {
        let log = AuditLog::new(100);
        let agent_id = AgentId::new();

        let entry = log.append(
            agent_id,
            "fs_read".to_string(),
            SecurityDecision::Allow,
            None,
        );

        assert!(!entry.hash.is_empty());
        assert_eq!(entry.prev_hash, "0".repeat(64));
        assert_eq!(entry.action, "fs_read");
    }

    #[test]
    fn chain_links_prev_hash() {
        let log = AuditLog::new(100);
        let agent_id = AgentId::new();

        let e1 = log.append(
            agent_id,
            "action1".to_string(),
            SecurityDecision::Allow,
            None,
        );
        let e2 = log.append(
            agent_id,
            "action2".to_string(),
            SecurityDecision::Deny {
                reason: "test".to_string(),
            },
            None,
        );

        assert_eq!(e2.prev_hash, e1.hash);
    }

    #[test]
    fn verify_chain_valid() {
        let log = AuditLog::new(100);
        let agent_id = AgentId::new();

        for i in 0..5 {
            log.append(
                agent_id,
                format!("action_{}", i),
                SecurityDecision::Allow,
                None,
            );
        }

        assert!(log.verify_chain());
    }

    #[test]
    fn verify_chain_empty() {
        let log = AuditLog::new(100);
        assert!(log.verify_chain());
    }

    #[test]
    fn respects_max_entries() {
        let log = AuditLog::new(3);
        let agent_id = AgentId::new();

        for i in 0..5 {
            log.append(
                agent_id,
                format!("action_{}", i),
                SecurityDecision::Allow,
                None,
            );
        }

        assert_eq!(log.len(), 3);
        let entries = log.entries();
        assert_eq!(entries[0].action, "action_2");
    }

    #[test]
    fn entries_returns_all() {
        let log = AuditLog::new(100);
        let agent_id = AgentId::new();

        log.append(agent_id, "a".to_string(), SecurityDecision::Allow, None);
        log.append(agent_id, "b".to_string(), SecurityDecision::Allow, None);

        let entries = log.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].action, "a");
        assert_eq!(entries[1].action, "b");
    }

    #[test]
    fn is_empty_and_len() {
        let log = AuditLog::new(100);
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);

        log.append(
            AgentId::new(),
            "test".to_string(),
            SecurityDecision::Allow,
            None,
        );
        assert!(!log.is_empty());
        assert_eq!(log.len(), 1);
    }

    #[test]
    fn stores_details() {
        let log = AuditLog::new(100);
        let details = serde_json::json!({"file": "/tmp/test.txt"});

        let entry = log.append(
            AgentId::new(),
            "fs_read".to_string(),
            SecurityDecision::Allow,
            Some(details.clone()),
        );

        assert_eq!(entry.details, Some(details));
    }

    #[test]
    fn stores_deny_decision() {
        let log = AuditLog::new(100);
        let decision = SecurityDecision::Deny {
            reason: "unauthorized".to_string(),
        };

        let entry = log.append(
            AgentId::new(),
            "exec_shell".to_string(),
            decision.clone(),
            None,
        );

        assert_eq!(entry.decision, decision);
    }
}
