//! Permission repository and Permission Gate for ClawX v0.2.
//!
//! Provides SQLite-backed storage for permission profiles and change events,
//! plus a `PermissionGate` that implements `PermissionGatePort` with the
//! risk-level decision logic from autonomy-architecture.md section 6.4.

use async_trait::async_trait;
use chrono::Utc;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::AgentId;
use clawx_types::permission::{
    CapabilityDimension, CapabilityScores, PermissionDecision, PermissionEvent, PermissionProfile,
    RiskLevel, TrustLevel,
};
use clawx_types::traits::PermissionGatePort;
use sqlx::SqlitePool;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Row types
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct ProfileRow {
    agent_id: String,
    capability_scores: String,
    safety_incidents: i64,
    last_downgraded_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ProfileRow> for PermissionProfile {
    type Error = ClawxError;

    fn try_from(row: ProfileRow) -> Result<Self> {
        Ok(PermissionProfile {
            agent_id: AgentId::from_str(&row.agent_id)
                .map_err(|e| ClawxError::Database(format!("invalid agent id: {}", e)))?,
            capability_scores: serde_json::from_str(&row.capability_scores)
                .map_err(|e| ClawxError::Database(format!("invalid capability_scores json: {}", e)))?,
            safety_incidents: row.safety_incidents as u32,
            last_downgraded_at: row
                .last_downgraded_at
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| ClawxError::Database(format!("invalid last_downgraded_at: {}", e)))
                })
                .transpose()?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid updated_at: {}", e)))?,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct EventRow {
    id: String,
    agent_id: String,
    capability: String,
    old_level: String,
    new_level: String,
    reason: String,
    run_id: Option<String>,
    created_at: String,
}

impl TryFrom<EventRow> for PermissionEvent {
    type Error = ClawxError;

    fn try_from(row: EventRow) -> Result<Self> {
        use clawx_types::autonomy::RunId;
        Ok(PermissionEvent {
            id: uuid::Uuid::parse_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid event id: {}", e)))?,
            agent_id: AgentId::from_str(&row.agent_id)
                .map_err(|e| ClawxError::Database(format!("invalid agent id: {}", e)))?,
            capability: CapabilityDimension::from_str(&row.capability)
                .map_err(|e| ClawxError::Database(format!("invalid capability: {}", e)))?,
            old_level: TrustLevel::from_str(&row.old_level)
                .map_err(|e| ClawxError::Database(format!("invalid old_level: {}", e)))?,
            new_level: TrustLevel::from_str(&row.new_level)
                .map_err(|e| ClawxError::Database(format!("invalid new_level: {}", e)))?,
            reason: row.reason,
            run_id: row
                .run_id
                .map(|s| {
                    RunId::from_str(&s)
                        .map_err(|e| ClawxError::Database(format!("invalid run_id: {}", e)))
                })
                .transpose()?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
        })
    }
}

// ---------------------------------------------------------------------------
// SqlitePermissionRepo — raw database operations
// ---------------------------------------------------------------------------

/// Repository for permission profiles and events backed by SQLite.
pub struct SqlitePermissionRepo;

impl SqlitePermissionRepo {
    /// Insert a new permission profile with the given capability scores.
    /// All trust levels default to L0 unless overridden in `scores`.
    pub async fn create_profile(
        pool: &SqlitePool,
        agent_id: &AgentId,
        scores: &CapabilityScores,
    ) -> Result<PermissionProfile> {
        let now = Utc::now().to_rfc3339();
        let scores_json = serde_json::to_string(scores)
            .map_err(|e| ClawxError::Internal(format!("serialize capability_scores: {}", e)))?;

        sqlx::query(
            "INSERT INTO permission_profiles (agent_id, capability_scores, safety_incidents, created_at, updated_at)
             VALUES (?, ?, 0, ?, ?)",
        )
        .bind(agent_id.to_string())
        .bind(&scores_json)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("create permission profile: {}", e)))?;

        Self::get_profile(pool, agent_id)
            .await?
            .ok_or_else(|| ClawxError::Internal("profile not found after insert".into()))
    }

    /// Fetch a permission profile by agent ID.
    pub async fn get_profile(
        pool: &SqlitePool,
        agent_id: &AgentId,
    ) -> Result<Option<PermissionProfile>> {
        let row: Option<ProfileRow> =
            sqlx::query_as("SELECT * FROM permission_profiles WHERE agent_id = ?")
                .bind(agent_id.to_string())
                .fetch_optional(pool)
                .await
                .map_err(|e| ClawxError::Database(format!("get permission profile: {}", e)))?;

        row.map(PermissionProfile::try_from).transpose()
    }

    /// Update a specific capability dimension and log a permission event.
    pub async fn update_capability(
        pool: &SqlitePool,
        agent_id: &AgentId,
        dimension: CapabilityDimension,
        new_level: TrustLevel,
        reason: &str,
    ) -> Result<()> {
        let profile = Self::get_profile(pool, agent_id)
            .await?
            .ok_or_else(|| ClawxError::NotFound {
                entity: "permission_profile".into(),
                id: agent_id.to_string(),
            })?;

        let old_level = profile.capability_scores.get(dimension);

        // Update the scores
        let mut scores = profile.capability_scores.clone();
        scores.set(dimension, new_level);
        let scores_json = serde_json::to_string(&scores)
            .map_err(|e| ClawxError::Internal(format!("serialize capability_scores: {}", e)))?;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE permission_profiles SET capability_scores = ?, updated_at = ? WHERE agent_id = ?",
        )
        .bind(&scores_json)
        .bind(&now)
        .bind(agent_id.to_string())
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("update capability: {}", e)))?;

        // Log the event
        let event_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO permission_events (id, agent_id, capability, old_level, new_level, reason, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&event_id)
        .bind(agent_id.to_string())
        .bind(dimension.to_string())
        .bind(old_level.to_string())
        .bind(new_level.to_string())
        .bind(reason)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("insert permission event: {}", e)))?;

        Ok(())
    }

    /// Increment the safety_incidents counter for an agent.
    pub async fn record_safety_incident(
        pool: &SqlitePool,
        agent_id: &AgentId,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE permission_profiles SET safety_incidents = safety_incidents + 1, updated_at = ? WHERE agent_id = ?",
        )
        .bind(&now)
        .bind(agent_id.to_string())
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("record safety incident: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(ClawxError::NotFound {
                entity: "permission_profile".into(),
                id: agent_id.to_string(),
            });
        }
        Ok(())
    }

    /// List all permission change events for an agent, newest first.
    pub async fn get_events(
        pool: &SqlitePool,
        agent_id: &AgentId,
    ) -> Result<Vec<PermissionEvent>> {
        let rows: Vec<EventRow> = sqlx::query_as(
            "SELECT * FROM permission_events WHERE agent_id = ? ORDER BY created_at DESC",
        )
        .bind(agent_id.to_string())
        .fetch_all(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("get permission events: {}", e)))?;

        rows.into_iter().map(PermissionEvent::try_from).collect()
    }
}

// ---------------------------------------------------------------------------
// PermissionGate — implements PermissionGatePort trait
// ---------------------------------------------------------------------------

/// Permission gate that checks agent trust levels against operation risk.
///
/// Decision logic (autonomy-architecture.md section 6.4):
/// - `Read`       => auto_allow if knowledge_read >= L1
/// - `Write`      => auto_allow if workspace_write >= L2
/// - `Send`       => auto_allow if external_send >= L3
/// - `MemoryLow`  => auto_allow if memory_write >= L2
/// - `MemoryHigh` => always Confirm
/// - `Danger`     => always Confirm
pub struct PermissionGate {
    pool: SqlitePool,
}

impl PermissionGate {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PermissionGatePort for PermissionGate {
    async fn check_permission(
        &self,
        agent_id: &AgentId,
        risk_level: RiskLevel,
    ) -> Result<PermissionDecision> {
        let profile = SqlitePermissionRepo::get_profile(&self.pool, agent_id)
            .await?
            .ok_or_else(|| ClawxError::NotFound {
                entity: "permission_profile".into(),
                id: agent_id.to_string(),
            })?;

        let scores = &profile.capability_scores;

        let decision = match risk_level {
            RiskLevel::Read => {
                if scores.knowledge_read >= TrustLevel::L1ReadTrusted {
                    PermissionDecision::AutoAllow
                } else {
                    PermissionDecision::Confirm {
                        reason: "knowledge_read trust level below L1".into(),
                    }
                }
            }
            RiskLevel::Write => {
                if scores.workspace_write >= TrustLevel::L2WorkspaceTrusted {
                    PermissionDecision::AutoAllow
                } else {
                    PermissionDecision::Confirm {
                        reason: "workspace_write trust level below L2".into(),
                    }
                }
            }
            RiskLevel::Send => {
                if scores.external_send >= TrustLevel::L3ChannelTrusted {
                    PermissionDecision::AutoAllow
                } else {
                    PermissionDecision::Confirm {
                        reason: "external_send trust level below L3".into(),
                    }
                }
            }
            RiskLevel::MemoryLow => {
                if scores.memory_write >= TrustLevel::L2WorkspaceTrusted {
                    PermissionDecision::AutoAllow
                } else {
                    PermissionDecision::Confirm {
                        reason: "memory_write trust level below L2".into(),
                    }
                }
            }
            RiskLevel::MemoryHigh => PermissionDecision::Confirm {
                reason: "memory_high operations always require confirmation".into(),
            },
            RiskLevel::Danger => PermissionDecision::Confirm {
                reason: "danger operations always require confirmation".into(),
            },
        };

        Ok(decision)
    }

    async fn get_profile(&self, agent_id: &AgentId) -> Result<Option<PermissionProfile>> {
        SqlitePermissionRepo::get_profile(&self.pool, agent_id).await
    }

    async fn update_profile(
        &self,
        agent_id: &AgentId,
        dimension: CapabilityDimension,
        new_level: TrustLevel,
        reason: String,
    ) -> Result<()> {
        SqlitePermissionRepo::update_capability(&self.pool, agent_id, dimension, new_level, &reason)
            .await
    }

    async fn record_safety_incident(&self, agent_id: &AgentId) -> Result<()> {
        SqlitePermissionRepo::record_safety_incident(&self.pool, agent_id).await
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    /// Helper: create an in-memory DB and insert a dummy agent row so the
    /// foreign-key constraint on permission_profiles is satisfied.
    async fn setup() -> (Database, AgentId) {
        let db = Database::in_memory().await.unwrap();
        let agent_id = AgentId::new();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, 'test-agent', 'assistant', 'default', 'idle', '[]', ?, ?)",
        )
        .bind(agent_id.to_string())
        .bind(&now)
        .bind(&now)
        .execute(&db.main)
        .await
        .unwrap();
        (db, agent_id)
    }

    // -----------------------------------------------------------------------
    // 1. create_profile — happy path with default L0 scores
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn create_profile_with_defaults() {
        let (db, agent_id) = setup().await;
        let scores = CapabilityScores::default();

        let profile = SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        assert_eq!(profile.agent_id, agent_id);
        assert_eq!(profile.safety_incidents, 0);
        assert_eq!(profile.capability_scores.knowledge_read, TrustLevel::L0Restricted);
        assert_eq!(profile.capability_scores.workspace_write, TrustLevel::L0Restricted);
        assert_eq!(profile.capability_scores.external_send, TrustLevel::L0Restricted);
        assert_eq!(profile.capability_scores.memory_write, TrustLevel::L0Restricted);
        assert_eq!(profile.capability_scores.shell_exec, TrustLevel::L0Restricted);
        assert!(profile.last_downgraded_at.is_none());
    }

    // -----------------------------------------------------------------------
    // 2. create_profile — with custom initial scores
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn create_profile_with_custom_scores() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.knowledge_read = TrustLevel::L1ReadTrusted;
        scores.workspace_write = TrustLevel::L2WorkspaceTrusted;

        let profile = SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        assert_eq!(profile.capability_scores.knowledge_read, TrustLevel::L1ReadTrusted);
        assert_eq!(profile.capability_scores.workspace_write, TrustLevel::L2WorkspaceTrusted);
        assert_eq!(profile.capability_scores.external_send, TrustLevel::L0Restricted);
    }

    // -----------------------------------------------------------------------
    // 3. get_profile — returns None for unknown agent
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn get_profile_returns_none_for_unknown() {
        let (db, _) = setup().await;
        let unknown = AgentId::new();

        let result = SqlitePermissionRepo::get_profile(&db.main, &unknown)
            .await
            .unwrap();

        assert!(result.is_none());
    }

    // -----------------------------------------------------------------------
    // 4. update_capability — changes score and logs event
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn update_capability_changes_score_and_logs_event() {
        let (db, agent_id) = setup().await;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &CapabilityScores::default())
            .await
            .unwrap();

        SqlitePermissionRepo::update_capability(
            &db.main,
            &agent_id,
            CapabilityDimension::KnowledgeRead,
            TrustLevel::L1ReadTrusted,
            "passed read audit",
        )
        .await
        .unwrap();

        // Verify the score was updated
        let profile = SqlitePermissionRepo::get_profile(&db.main, &agent_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(profile.capability_scores.knowledge_read, TrustLevel::L1ReadTrusted);

        // Verify an event was logged
        let events = SqlitePermissionRepo::get_events(&db.main, &agent_id)
            .await
            .unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].capability, CapabilityDimension::KnowledgeRead);
        assert_eq!(events[0].old_level, TrustLevel::L0Restricted);
        assert_eq!(events[0].new_level, TrustLevel::L1ReadTrusted);
        assert_eq!(events[0].reason, "passed read audit");
    }

    // -----------------------------------------------------------------------
    // 5. update_capability — returns NotFound for missing profile
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn update_capability_not_found_for_missing_profile() {
        let (db, _) = setup().await;
        let unknown = AgentId::new();

        let result = SqlitePermissionRepo::update_capability(
            &db.main,
            &unknown,
            CapabilityDimension::KnowledgeRead,
            TrustLevel::L1ReadTrusted,
            "should fail",
        )
        .await;

        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    // -----------------------------------------------------------------------
    // 6. record_safety_incident — increments counter
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn record_safety_incident_increments_counter() {
        let (db, agent_id) = setup().await;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &CapabilityScores::default())
            .await
            .unwrap();

        SqlitePermissionRepo::record_safety_incident(&db.main, &agent_id)
            .await
            .unwrap();
        SqlitePermissionRepo::record_safety_incident(&db.main, &agent_id)
            .await
            .unwrap();

        let profile = SqlitePermissionRepo::get_profile(&db.main, &agent_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(profile.safety_incidents, 2);
    }

    // -----------------------------------------------------------------------
    // 7. record_safety_incident — returns NotFound for missing profile
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn record_safety_incident_not_found_for_missing_profile() {
        let (db, _) = setup().await;
        let unknown = AgentId::new();

        let result = SqlitePermissionRepo::record_safety_incident(&db.main, &unknown).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    // -----------------------------------------------------------------------
    // 8. check_permission — read risk auto-allowed at L1
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_read_auto_allow_at_l1() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.knowledge_read = TrustLevel::L1ReadTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate.check_permission(&agent_id, RiskLevel::Read).await.unwrap();
        assert_eq!(decision, PermissionDecision::AutoAllow);
    }

    // -----------------------------------------------------------------------
    // 9. check_permission — read risk requires confirm at L0
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_read_confirm_at_l0() {
        let (db, agent_id) = setup().await;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &CapabilityScores::default())
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate.check_permission(&agent_id, RiskLevel::Read).await.unwrap();
        assert!(matches!(decision, PermissionDecision::Confirm { .. }));
    }

    // -----------------------------------------------------------------------
    // 10. check_permission — write risk auto-allowed at L2
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_write_auto_allow_at_l2() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.workspace_write = TrustLevel::L2WorkspaceTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate.check_permission(&agent_id, RiskLevel::Write).await.unwrap();
        assert_eq!(decision, PermissionDecision::AutoAllow);
    }

    // -----------------------------------------------------------------------
    // 11. check_permission — write risk confirm at L1
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_write_confirm_below_l2() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.workspace_write = TrustLevel::L1ReadTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate.check_permission(&agent_id, RiskLevel::Write).await.unwrap();
        assert!(matches!(decision, PermissionDecision::Confirm { .. }));
    }

    // -----------------------------------------------------------------------
    // 12. check_permission — send risk auto-allowed at L3
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_send_auto_allow_at_l3() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.external_send = TrustLevel::L3ChannelTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate.check_permission(&agent_id, RiskLevel::Send).await.unwrap();
        assert_eq!(decision, PermissionDecision::AutoAllow);
    }

    // -----------------------------------------------------------------------
    // 13. check_permission — send risk confirm below L3
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_send_confirm_below_l3() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.external_send = TrustLevel::L2WorkspaceTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate.check_permission(&agent_id, RiskLevel::Send).await.unwrap();
        assert!(matches!(decision, PermissionDecision::Confirm { .. }));
    }

    // -----------------------------------------------------------------------
    // 14. check_permission — memory_low auto-allowed at L2
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_memory_low_auto_allow_at_l2() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.memory_write = TrustLevel::L2WorkspaceTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate
            .check_permission(&agent_id, RiskLevel::MemoryLow)
            .await
            .unwrap();
        assert_eq!(decision, PermissionDecision::AutoAllow);
    }

    // -----------------------------------------------------------------------
    // 15. check_permission — memory_high always confirm
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_memory_high_always_confirm() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        // Even with all-max trust, memory_high still requires confirmation
        scores.memory_write = TrustLevel::L3ChannelTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate
            .check_permission(&agent_id, RiskLevel::MemoryHigh)
            .await
            .unwrap();
        assert!(matches!(decision, PermissionDecision::Confirm { .. }));
    }

    // -----------------------------------------------------------------------
    // 16. check_permission — danger always confirm
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_danger_always_confirm() {
        let (db, agent_id) = setup().await;
        let mut scores = CapabilityScores::default();
        scores.shell_exec = TrustLevel::L3ChannelTrusted;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &scores)
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        let decision = gate
            .check_permission(&agent_id, RiskLevel::Danger)
            .await
            .unwrap();
        assert!(matches!(decision, PermissionDecision::Confirm { .. }));
    }

    // -----------------------------------------------------------------------
    // 17. check_permission — returns error for nonexistent profile
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn check_permission_error_for_missing_profile() {
        let (db, _) = setup().await;
        let unknown = AgentId::new();

        let gate = PermissionGate::new(db.main.clone());
        let result = gate.check_permission(&unknown, RiskLevel::Read).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    // -----------------------------------------------------------------------
    // 18. get_events — multiple events ordered newest first
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn get_events_returns_newest_first() {
        let (db, agent_id) = setup().await;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &CapabilityScores::default())
            .await
            .unwrap();

        SqlitePermissionRepo::update_capability(
            &db.main,
            &agent_id,
            CapabilityDimension::KnowledgeRead,
            TrustLevel::L1ReadTrusted,
            "first change",
        )
        .await
        .unwrap();

        SqlitePermissionRepo::update_capability(
            &db.main,
            &agent_id,
            CapabilityDimension::WorkspaceWrite,
            TrustLevel::L2WorkspaceTrusted,
            "second change",
        )
        .await
        .unwrap();

        let events = SqlitePermissionRepo::get_events(&db.main, &agent_id)
            .await
            .unwrap();
        assert_eq!(events.len(), 2);
        // Newest first
        assert_eq!(events[0].reason, "second change");
        assert_eq!(events[1].reason, "first change");
    }

    // -----------------------------------------------------------------------
    // 19. get_events — empty for agent with no changes
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn get_events_empty_for_no_changes() {
        let (db, agent_id) = setup().await;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &CapabilityScores::default())
            .await
            .unwrap();

        let events = SqlitePermissionRepo::get_events(&db.main, &agent_id)
            .await
            .unwrap();
        assert!(events.is_empty());
    }

    // -----------------------------------------------------------------------
    // 20. PermissionGate trait — update_profile delegates correctly
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn gate_update_profile_delegates_to_repo() {
        let (db, agent_id) = setup().await;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &CapabilityScores::default())
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        gate.update_profile(
            &agent_id,
            CapabilityDimension::ExternalSend,
            TrustLevel::L3ChannelTrusted,
            "promoted via gate".to_string(),
        )
        .await
        .unwrap();

        let profile = gate.get_profile(&agent_id).await.unwrap().unwrap();
        assert_eq!(profile.capability_scores.external_send, TrustLevel::L3ChannelTrusted);
    }

    // -----------------------------------------------------------------------
    // 21. PermissionGate trait — record_safety_incident delegates correctly
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn gate_record_safety_incident_delegates() {
        let (db, agent_id) = setup().await;
        SqlitePermissionRepo::create_profile(&db.main, &agent_id, &CapabilityScores::default())
            .await
            .unwrap();

        let gate = PermissionGate::new(db.main.clone());
        gate.record_safety_incident(&agent_id).await.unwrap();

        let profile = gate.get_profile(&agent_id).await.unwrap().unwrap();
        assert_eq!(profile.safety_incidents, 1);
    }
}
