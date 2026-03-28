//! Run recovery: detects and handles orphaned runs after service restart.
//!
//! On startup, queries for incomplete runs and applies recovery policy:
//! - `running` / `planning`: retry if attempts remain, else mark `failed`
//! - `queued`: leave as-is for re-execution
//! - `waiting_confirmation`: mark `interrupted`

use chrono::Utc;
use clawx_types::autonomy::*;
use clawx_types::error::Result;
use clawx_types::traits::{RunUpdate, TaskRegistryPort};

/// Configuration for run recovery.
#[derive(Debug, Clone)]
pub struct RunRecoveryConfig {
    /// Maximum retry attempts for failed runs (default: 3).
    pub max_retries: u32,
    /// Base delay between retries in seconds (exponential backoff).
    pub retry_base_delay_secs: u64,
}

impl Default for RunRecoveryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_base_delay_secs: 5,
        }
    }
}

/// Result of a recovery scan.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    pub orphaned_found: u32,
    pub marked_failed: u32,
    pub marked_interrupted: u32,
    pub left_queued: u32,
    pub retries_scheduled: u32,
}

/// Recover orphaned runs after service restart.
///
/// Scans for incomplete runs and applies recovery policy based on their status
/// and retry configuration.
pub async fn recover_orphaned_runs(
    registry: &dyn TaskRegistryPort,
    config: &RunRecoveryConfig,
) -> Result<RecoveryReport> {
    let incomplete = registry.get_incomplete_runs().await?;
    let mut report = RecoveryReport::default();
    report.orphaned_found = incomplete.len() as u32;

    for run in incomplete {
        match run.run_status {
            RunStatus::Running | RunStatus::Planning => {
                if run.attempt < config.max_retries {
                    report.retries_scheduled += 1;
                    registry
                        .update_run(
                            run.id,
                            RunUpdate {
                                run_status: Some(RunStatus::Queued),
                                failure_reason: Some(format!(
                                    "service restart recovery - retry {} of {}",
                                    run.attempt + 1,
                                    config.max_retries
                                )),
                                ..Default::default()
                            },
                        )
                        .await?;
                } else {
                    report.marked_failed += 1;
                    registry
                        .update_run(
                            run.id,
                            RunUpdate {
                                run_status: Some(RunStatus::Failed),
                                failure_reason: Some(
                                    "service restart - max retries exceeded".into(),
                                ),
                                finished_at: Some(Utc::now()),
                                ..Default::default()
                            },
                        )
                        .await?;
                }
            }
            RunStatus::Queued => {
                report.left_queued += 1;
                // Leave as-is for re-execution
            }
            RunStatus::WaitingConfirmation => {
                report.marked_interrupted += 1;
                registry
                    .update_run(
                        run.id,
                        RunUpdate {
                            run_status: Some(RunStatus::Interrupted),
                            failure_reason: Some(
                                "service restart - confirmation interrupted".into(),
                            ),
                            finished_at: Some(Utc::now()),
                            ..Default::default()
                        },
                    )
                    .await?;
            }
            _ => {
                // Already completed/failed/interrupted - should not appear in incomplete list
            }
        }
    }

    Ok(report)
}

/// Calculate exponential backoff delay for a retry attempt.
///
/// Uses `base_delay_secs * 2^(attempt - 1)`, capped at 300 seconds (5 minutes).
pub fn backoff_delay(attempt: u32, base_delay_secs: u64) -> std::time::Duration {
    let delay = base_delay_secs * 2u64.pow(attempt.saturating_sub(1));
    std::time::Duration::from_secs(delay.min(300))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::task_repo::SqliteTaskRegistry;
    use clawx_types::autonomy::*;
    use clawx_types::ids::{AgentId, TaskId};
    use clawx_types::traits::TaskRegistryPort;
    use chrono::Utc;
    use std::str::FromStr;

    const TEST_AGENT_ID: &str = "00000000-0000-0000-0000-000000000001";

    async fn setup() -> (Database, SqliteTaskRegistry) {
        let db = Database::in_memory().await.unwrap();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'idle', '[]', ?, ?)",
        )
        .bind(TEST_AGENT_ID)
        .bind("TestAgent")
        .bind("assistant")
        .bind("default")
        .bind(&now)
        .bind(&now)
        .execute(&db.main)
        .await
        .unwrap();

        let registry = SqliteTaskRegistry::new(db.main.clone());
        (db, registry)
    }

    fn make_task() -> Task {
        let now = Utc::now();
        Task {
            id: TaskId::new(),
            agent_id: AgentId::from_str(TEST_AGENT_ID).unwrap(),
            name: "RecoveryTestTask".to_string(),
            goal: "Test goal".to_string(),
            source_kind: TaskSourceKind::Manual,
            lifecycle_status: TaskLifecycleStatus::Active,
            default_max_steps: 10,
            default_timeout_secs: 1800,
            notification_policy: serde_json::json!({}),
            suppression_state: SuppressionState::Normal,
            last_run_at: None,
            next_run_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_run(task_id: TaskId, key: &str, status: RunStatus, attempt: u32) -> Run {
        let now = Utc::now();
        Run {
            id: RunId::new(),
            task_id,
            trigger_id: None,
            idempotency_key: key.to_string(),
            run_status: status,
            attempt,
            lease_owner: None,
            lease_expires_at: None,
            checkpoint: serde_json::json!({}),
            tokens_used: 0,
            steps_count: 0,
            result_summary: None,
            failure_reason: None,
            feedback_kind: None,
            feedback_reason: None,
            notification_status: NotificationStatus::Pending,
            triggered_at: now,
            started_at: None,
            finished_at: None,
            created_at: now,
        }
    }

    // -----------------------------------------------------------------------
    // 1. Running run with max retries exceeded -> failed
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn recover_running_run_marks_failed() {
        let (_db, reg) = setup().await;
        let task = make_task();
        let task_id = reg.create_task(task).await.unwrap();

        // Create a running run at attempt 3 (max retries = 3)
        let run = make_run(task_id, "key-running-fail", RunStatus::Running, 3);
        let run_id = run.id;
        reg.create_run(run).await.unwrap();

        let config = RunRecoveryConfig {
            max_retries: 3,
            retry_base_delay_secs: 5,
        };

        let report = recover_orphaned_runs(&reg, &config).await.unwrap();

        assert_eq!(report.orphaned_found, 1);
        assert_eq!(report.marked_failed, 1);
        assert_eq!(report.retries_scheduled, 0);

        let updated = reg.get_run(run_id).await.unwrap().unwrap();
        assert_eq!(updated.run_status, RunStatus::Failed);
        assert!(updated.failure_reason.as_deref().unwrap().contains("max retries exceeded"));
        assert!(updated.finished_at.is_some());
    }

    // -----------------------------------------------------------------------
    // 2. Running run with retries available -> queued for retry
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn recover_running_run_retries() {
        let (_db, reg) = setup().await;
        let task = make_task();
        let task_id = reg.create_task(task).await.unwrap();

        // Create a running run at attempt 1 (max retries = 3)
        let run = make_run(task_id, "key-running-retry", RunStatus::Running, 1);
        let run_id = run.id;
        reg.create_run(run).await.unwrap();

        let config = RunRecoveryConfig {
            max_retries: 3,
            retry_base_delay_secs: 5,
        };

        let report = recover_orphaned_runs(&reg, &config).await.unwrap();

        assert_eq!(report.orphaned_found, 1);
        assert_eq!(report.retries_scheduled, 1);
        assert_eq!(report.marked_failed, 0);

        let updated = reg.get_run(run_id).await.unwrap().unwrap();
        assert_eq!(updated.run_status, RunStatus::Queued);
        assert!(updated.failure_reason.as_deref().unwrap().contains("retry 2 of 3"));
    }

    // -----------------------------------------------------------------------
    // 3. Queued run left as-is
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn recover_queued_run_left_as_is() {
        let (_db, reg) = setup().await;
        let task = make_task();
        let task_id = reg.create_task(task).await.unwrap();

        let run = make_run(task_id, "key-queued", RunStatus::Queued, 1);
        let run_id = run.id;
        reg.create_run(run).await.unwrap();

        let config = RunRecoveryConfig::default();
        let report = recover_orphaned_runs(&reg, &config).await.unwrap();

        assert_eq!(report.orphaned_found, 1);
        assert_eq!(report.left_queued, 1);
        assert_eq!(report.marked_failed, 0);
        assert_eq!(report.retries_scheduled, 0);

        let updated = reg.get_run(run_id).await.unwrap().unwrap();
        assert_eq!(updated.run_status, RunStatus::Queued);
    }

    // -----------------------------------------------------------------------
    // 4. WaitingConfirmation -> interrupted
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn recover_waiting_confirmation_marks_interrupted() {
        let (_db, reg) = setup().await;
        let task = make_task();
        let task_id = reg.create_task(task).await.unwrap();

        let run = make_run(task_id, "key-waiting", RunStatus::WaitingConfirmation, 1);
        let run_id = run.id;
        reg.create_run(run).await.unwrap();

        let config = RunRecoveryConfig::default();
        let report = recover_orphaned_runs(&reg, &config).await.unwrap();

        assert_eq!(report.orphaned_found, 1);
        assert_eq!(report.marked_interrupted, 1);

        let updated = reg.get_run(run_id).await.unwrap().unwrap();
        assert_eq!(updated.run_status, RunStatus::Interrupted);
        assert!(updated.failure_reason.as_deref().unwrap().contains("confirmation interrupted"));
        assert!(updated.finished_at.is_some());
    }

    // -----------------------------------------------------------------------
    // 5. No orphans -> empty report
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn recover_no_orphans_returns_empty_report() {
        let (_db, reg) = setup().await;

        let config = RunRecoveryConfig::default();
        let report = recover_orphaned_runs(&reg, &config).await.unwrap();

        assert_eq!(report, RecoveryReport::default());
    }

    // -----------------------------------------------------------------------
    // 6. Mixed orphans all handled correctly
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn recover_mixed_orphans() {
        let (_db, reg) = setup().await;
        let task = make_task();
        let task_id = reg.create_task(task).await.unwrap();

        // queued run
        reg.create_run(make_run(task_id, "key-q", RunStatus::Queued, 1))
            .await
            .unwrap();
        // running run with retries available (attempt 1 of 3)
        reg.create_run(make_run(task_id, "key-r-retry", RunStatus::Running, 1))
            .await
            .unwrap();
        // planning run with max retries exceeded (attempt 3 of 3)
        reg.create_run(make_run(task_id, "key-p-fail", RunStatus::Planning, 3))
            .await
            .unwrap();
        // waiting_confirmation run
        reg.create_run(make_run(task_id, "key-wc", RunStatus::WaitingConfirmation, 1))
            .await
            .unwrap();

        let config = RunRecoveryConfig {
            max_retries: 3,
            retry_base_delay_secs: 5,
        };

        let report = recover_orphaned_runs(&reg, &config).await.unwrap();

        assert_eq!(report.orphaned_found, 4);
        assert_eq!(report.left_queued, 1);
        assert_eq!(report.retries_scheduled, 1);
        assert_eq!(report.marked_failed, 1);
        assert_eq!(report.marked_interrupted, 1);
    }

    // -----------------------------------------------------------------------
    // 7. Backoff delay: exponential
    // -----------------------------------------------------------------------
    #[test]
    fn backoff_delay_exponential() {
        // base_delay=5, attempt 1 => 5 * 2^0 = 5s
        assert_eq!(backoff_delay(1, 5), std::time::Duration::from_secs(5));
        // attempt 2 => 5 * 2^1 = 10s
        assert_eq!(backoff_delay(2, 5), std::time::Duration::from_secs(10));
        // attempt 3 => 5 * 2^2 = 20s
        assert_eq!(backoff_delay(3, 5), std::time::Duration::from_secs(20));
        // attempt 4 => 5 * 2^3 = 40s
        assert_eq!(backoff_delay(4, 5), std::time::Duration::from_secs(40));
    }

    // -----------------------------------------------------------------------
    // 8. Backoff delay: capped at 300s
    // -----------------------------------------------------------------------
    #[test]
    fn backoff_delay_capped() {
        // attempt 10 => 5 * 2^9 = 2560, capped at 300
        assert_eq!(backoff_delay(10, 5), std::time::Duration::from_secs(300));
        // attempt 20 => enormous, still capped at 300
        assert_eq!(backoff_delay(20, 5), std::time::Duration::from_secs(300));
    }
}
