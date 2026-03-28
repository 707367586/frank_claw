//! Cron scheduling for ClawX.
//!
//! Provides cron expression parsing, next-fire-time computation,
//! and a background `TaskScheduler` that scans for due triggers
//! and creates task runs.

use chrono::{DateTime, Utc};
use clawx_types::error::{ClawxError, Result};
use sqlx::SqlitePool;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Cron parsing utilities
// ---------------------------------------------------------------------------

/// Parse a cron expression string into a `cron::Schedule`.
///
/// The `cron` crate uses 6-field format: `sec min hour day_of_month month day_of_week`.
pub fn parse_cron(expr: &str) -> Result<cron::Schedule> {
    expr.parse::<cron::Schedule>()
        .map_err(|e| ClawxError::Validation(format!("invalid cron expression '{}': {}", expr, e)))
}

/// Compute the next fire time after a given point in time.
pub fn next_fire_time(expr: &str, after: DateTime<Utc>) -> Result<DateTime<Utc>> {
    let schedule = parse_cron(expr)?;
    schedule
        .after(&after)
        .next()
        .ok_or_else(|| ClawxError::Validation(format!("cron '{}' has no future occurrences", expr)))
}

/// Extract a cron expression from trigger config JSON and compute the next fire time from now.
///
/// Returns `Ok(None)` if the config does not contain a `"cron"` key.
pub fn compute_next_fire_at(trigger_config: &serde_json::Value) -> Result<Option<DateTime<Utc>>> {
    match trigger_config.get("cron").and_then(|v| v.as_str()) {
        Some(cron_expr) => {
            let next = next_fire_time(cron_expr, Utc::now())?;
            Ok(Some(next))
        }
        None => Ok(None),
    }
}

// ---------------------------------------------------------------------------
// TaskScheduler
// ---------------------------------------------------------------------------

/// Background scheduler that periodically scans for due triggers and creates runs.
pub struct TaskScheduler {
    pool: SqlitePool,
    scan_interval: Duration,
    running: Arc<AtomicBool>,
}

impl TaskScheduler {
    /// Create a new scheduler.
    pub fn new(pool: SqlitePool, scan_interval: Duration) -> Self {
        Self {
            pool,
            scan_interval,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Start the background scheduler loop. Returns a `JoinHandle` for the spawned task.
    pub fn start(&self) -> JoinHandle<()> {
        self.running.store(true, Ordering::SeqCst);
        let pool = self.pool.clone();
        let interval = self.scan_interval;
        let running = self.running.clone();

        tokio::spawn(async move {
            info!("TaskScheduler started, scan_interval={:?}", interval);
            while running.load(Ordering::SeqCst) {
                if let Err(e) = Self::tick(&pool).await {
                    error!("scheduler tick error: {}", e);
                }
                tokio::time::sleep(interval).await;
            }
            info!("TaskScheduler stopped");
        })
    }

    /// Stop the background scheduler.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Returns whether the scheduler is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Single tick: scan due triggers, create runs, update triggers.
    pub(crate) async fn tick(pool: &SqlitePool) -> Result<()> {
        use clawx_types::autonomy::*;

        let now = Utc::now();
        let now_str = now.to_rfc3339();

        // 1. Get due triggers
        let rows: Vec<TriggerRow> = sqlx::query_as(
            "SELECT * FROM task_triggers WHERE status = 'active' AND next_fire_at IS NOT NULL AND next_fire_at <= ?
             ORDER BY next_fire_at ASC",
        )
        .bind(&now_str)
        .fetch_all(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("scheduler get due triggers: {}", e)))?;

        let triggers: Vec<Trigger> = rows
            .into_iter()
            .map(|r| r.try_into())
            .collect::<Result<_>>()?;

        if triggers.is_empty() {
            return Ok(());
        }

        info!("scheduler found {} due trigger(s)", triggers.len());

        for trigger in &triggers {
            let fire_time = trigger
                .next_fire_at
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default();

            let idempotency_key = format!("{}:{}", trigger.id, fire_time);

            // Check idempotency: skip if run already exists
            let existing: Option<(String,)> =
                sqlx::query_as("SELECT id FROM task_runs WHERE idempotency_key = ?")
                    .bind(&idempotency_key)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| {
                        ClawxError::Database(format!("scheduler check idempotency: {}", e))
                    })?;

            if existing.is_some() {
                warn!(
                    "run already exists for idempotency_key={}, skipping",
                    idempotency_key
                );
            } else {
                // 2. Create a new run
                let run_id = RunId::new();
                let run_id_str = run_id.to_string();
                let task_id_str = trigger.task_id.to_string();
                let trigger_id_str = trigger.id.to_string();
                let now_rfc = now.to_rfc3339();

                sqlx::query(
                    "INSERT INTO task_runs (id, task_id, trigger_id, idempotency_key, run_status,
                     attempt, checkpoint, tokens_used, steps_count, notification_status,
                     triggered_at, created_at)
                     VALUES (?, ?, ?, ?, 'queued', 1, '{}', 0, 0, 'pending', ?, ?)",
                )
                .bind(&run_id_str)
                .bind(&task_id_str)
                .bind(&trigger_id_str)
                .bind(&idempotency_key)
                .bind(&now_rfc)
                .bind(&now_rfc)
                .execute(pool)
                .await
                .map_err(|e| ClawxError::Database(format!("scheduler create run: {}", e)))?;

                info!(
                    "created run {} for trigger {} (key={})",
                    run_id, trigger.id, idempotency_key
                );
            }

            // 3. Compute next fire time from cron config
            let next = match trigger
                .trigger_config
                .get("cron")
                .and_then(|v| v.as_str())
            {
                Some(cron_expr) => match next_fire_time(cron_expr, now) {
                    Ok(dt) => Some(dt),
                    Err(e) => {
                        warn!("failed to compute next fire time for trigger {}: {}", trigger.id, e);
                        None
                    }
                },
                None => None,
            };

            // 4. Update trigger: next_fire_at and last_fired_at
            let next_str = next.map(|dt| dt.to_rfc3339());
            let now_rfc = now.to_rfc3339();

            sqlx::query(
                "UPDATE task_triggers SET next_fire_at = ?, last_fired_at = ?, updated_at = ? WHERE id = ?",
            )
            .bind(&next_str)
            .bind(&now_rfc)
            .bind(&now_rfc)
            .bind(trigger.id.to_string())
            .execute(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("scheduler update trigger: {}", e)))?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Event trigger matching
// ---------------------------------------------------------------------------

/// Check if an event matches any active event triggers and create runs for them.
///
/// Queries all active triggers of kind `event`, checks if their `trigger_config.event_kind`
/// matches the incoming `event_kind`, and creates a run for each match.
pub async fn handle_event(
    pool: &SqlitePool,
    event_kind: &str,
    _event_data: &serde_json::Value,
) -> Result<Vec<clawx_types::autonomy::RunId>> {
    use clawx_types::autonomy::*;

    let now = Utc::now();
    let now_str = now.to_rfc3339();

    // Query active event triggers
    let rows: Vec<TriggerRow> = sqlx::query_as(
        "SELECT * FROM task_triggers WHERE trigger_kind = 'event' AND status = 'active'",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("handle_event get triggers: {}", e)))?;

    let triggers: Vec<Trigger> = rows
        .into_iter()
        .map(|r| r.try_into())
        .collect::<Result<_>>()?;

    let mut created_runs = Vec::new();

    for trigger in &triggers {
        if matches_event(&trigger.trigger_config, event_kind) {
            let idempotency_key = format!(
                "event:{}:{}:{}",
                trigger.id,
                event_kind,
                now.timestamp_millis()
            );

            // Check idempotency
            let existing: Option<(String,)> =
                sqlx::query_as("SELECT id FROM task_runs WHERE idempotency_key = ?")
                    .bind(&idempotency_key)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| {
                        ClawxError::Database(format!("handle_event check idempotency: {}", e))
                    })?;

            if existing.is_some() {
                continue;
            }

            let run_id = RunId::new();
            let run_id_str = run_id.to_string();
            let task_id_str = trigger.task_id.to_string();
            let trigger_id_str = trigger.id.to_string();

            sqlx::query(
                "INSERT INTO task_runs (id, task_id, trigger_id, idempotency_key, run_status,
                 attempt, checkpoint, tokens_used, steps_count, notification_status,
                 triggered_at, created_at)
                 VALUES (?, ?, ?, ?, 'queued', 1, '{}', 0, 0, 'pending', ?, ?)",
            )
            .bind(&run_id_str)
            .bind(&task_id_str)
            .bind(&trigger_id_str)
            .bind(&idempotency_key)
            .bind(&now_str)
            .bind(&now_str)
            .execute(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("handle_event create run: {}", e)))?;

            // Update trigger's last_fired_at
            sqlx::query(
                "UPDATE task_triggers SET last_fired_at = ?, updated_at = ? WHERE id = ?",
            )
            .bind(&now_str)
            .bind(&now_str)
            .bind(&trigger_id_str)
            .execute(pool)
            .await
            .map_err(|e| {
                ClawxError::Database(format!("handle_event update trigger: {}", e))
            })?;

            info!(
                "event trigger fired: run={} trigger={} event_kind={}",
                run_id, trigger.id, event_kind
            );

            created_runs.push(run_id);
        }
    }

    Ok(created_runs)
}

/// Check if a trigger's config matches an event kind.
///
/// The trigger_config is expected to contain `{"event_kind": "<kind>"}`.
pub fn matches_event(trigger_config: &serde_json::Value, event_kind: &str) -> bool {
    trigger_config
        .get("event_kind")
        .and_then(|v| v.as_str())
        .map(|k| k == event_kind)
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Internal row type for direct SQL queries (mirrors task_repo TriggerRow)
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct TriggerRow {
    id: String,
    task_id: String,
    trigger_kind: String,
    trigger_config: String,
    status: String,
    next_fire_at: Option<String>,
    last_fired_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<TriggerRow> for clawx_types::autonomy::Trigger {
    type Error = ClawxError;

    fn try_from(row: TriggerRow) -> Result<Self> {
        use clawx_types::autonomy::*;
        use clawx_types::ids::TaskId;
        use std::str::FromStr;

        Ok(clawx_types::autonomy::Trigger {
            id: TriggerId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid trigger id: {}", e)))?,
            task_id: TaskId::from_str(&row.task_id)
                .map_err(|e| ClawxError::Database(format!("invalid task id: {}", e)))?,
            trigger_kind: TriggerKind::from_str(&row.trigger_kind)
                .map_err(|e| ClawxError::Database(format!("invalid trigger_kind: {}", e)))?,
            trigger_config: serde_json::from_str(&row.trigger_config)
                .map_err(|e| ClawxError::Database(format!("invalid trigger_config: {}", e)))?,
            status: TriggerStatus::from_str(&row.status)
                .map_err(|e| ClawxError::Database(format!("invalid trigger status: {}", e)))?,
            next_fire_at: row
                .next_fire_at
                .as_ref()
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| ClawxError::Database(format!("invalid next_fire_at: {}", e)))
                })
                .transpose()?,
            last_fired_at: row
                .last_fired_at
                .as_ref()
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| ClawxError::Database(format!("invalid last_fired_at: {}", e)))
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    // -----------------------------------------------------------------------
    // Unit tests for cron parsing
    // -----------------------------------------------------------------------

    #[test]
    fn parse_cron_valid() {
        // 6-field cron: sec min hour day month day_of_week
        let schedule = parse_cron("0 * * * * *");
        assert!(schedule.is_ok(), "expected valid cron to parse, got {:?}", schedule.err());
    }

    #[test]
    fn parse_cron_invalid() {
        let result = parse_cron("invalid");
        assert!(result.is_err(), "expected invalid cron to fail");
    }

    #[test]
    fn next_fire_time_in_future() {
        let after = Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap();
        let next = next_fire_time("0 * * * * *", after).unwrap();
        assert!(
            next > after,
            "expected next fire time {} to be after {}",
            next,
            after
        );
    }

    #[test]
    fn next_fire_time_every_minute() {
        // "0 * * * * *" fires at second 0 of every minute
        let after = Utc.with_ymd_and_hms(2025, 6, 15, 12, 30, 30).unwrap();
        let next = next_fire_time("0 * * * * *", after).unwrap();
        // Should be 12:31:00
        assert_eq!(next.minute(), 31);
        assert_eq!(next.second(), 0);
    }

    #[test]
    fn compute_next_fire_at_from_config() {
        let config = serde_json::json!({"cron": "0 * * * * *"});
        let result = compute_next_fire_at(&config).unwrap();
        assert!(result.is_some(), "expected Some, got None");
        let next = result.unwrap();
        assert!(next > Utc::now(), "expected next fire time to be in the future");
    }

    #[test]
    fn compute_next_fire_at_missing_cron() {
        let config = serde_json::json!({"type": "event"});
        let result = compute_next_fire_at(&config).unwrap();
        assert!(result.is_none(), "expected None when no cron key");
    }

    #[test]
    fn compute_next_fire_at_invalid_cron() {
        let config = serde_json::json!({"cron": "not-a-cron"});
        let result = compute_next_fire_at(&config);
        assert!(result.is_err(), "expected error for invalid cron expression");
    }

    #[test]
    fn compute_next_fire_at_null_cron_value() {
        let config = serde_json::json!({"cron": null});
        let result = compute_next_fire_at(&config).unwrap();
        assert!(result.is_none(), "expected None when cron value is null");
    }

    #[test]
    fn compute_next_fire_at_empty_object() {
        let config = serde_json::json!({});
        let result = compute_next_fire_at(&config).unwrap();
        assert!(result.is_none(), "expected None for empty config");
    }

    // -----------------------------------------------------------------------
    // Integration test: scheduler creates runs for due triggers
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn scheduler_creates_runs_for_due_triggers() {
        use chrono::Duration;

        let db = clawx_runtime::db::Database::in_memory().await.unwrap();
        let pool = &db.main;

        // Insert a test agent
        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let agent_id = "00000000-0000-0000-0000-000000000001";

        sqlx::query(
            "INSERT INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'idle', '[]', ?, ?)",
        )
        .bind(agent_id)
        .bind("TestAgent")
        .bind("assistant")
        .bind("default")
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();

        // Insert a test task
        let task_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO tasks (id, agent_id, name, goal, source_kind, lifecycle_status,
             default_max_steps, default_timeout_secs, notification_policy, suppression_state,
             created_at, updated_at)
             VALUES (?, ?, ?, ?, 'manual', 'active', 10, 1800, '{}', 'normal', ?, ?)",
        )
        .bind(&task_id)
        .bind(agent_id)
        .bind("Test Task")
        .bind("Test goal")
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();

        // Insert a trigger with past next_fire_at (it is due)
        let trigger_id = uuid::Uuid::new_v4().to_string();
        let past = (now - Duration::hours(1)).to_rfc3339();
        let cron_config = serde_json::json!({"cron": "0 * * * * *"}).to_string();

        sqlx::query(
            "INSERT INTO task_triggers (id, task_id, trigger_kind, trigger_config, status,
             next_fire_at, created_at, updated_at)
             VALUES (?, ?, 'time', ?, 'active', ?, ?, ?)",
        )
        .bind(&trigger_id)
        .bind(&task_id)
        .bind(&cron_config)
        .bind(&past)
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();

        // Also insert a trigger that is NOT due (future next_fire_at)
        let trigger_id_future = uuid::Uuid::new_v4().to_string();
        let future = (now + Duration::hours(1)).to_rfc3339();

        sqlx::query(
            "INSERT INTO task_triggers (id, task_id, trigger_kind, trigger_config, status,
             next_fire_at, created_at, updated_at)
             VALUES (?, ?, 'time', ?, 'active', ?, ?, ?)",
        )
        .bind(&trigger_id_future)
        .bind(&task_id)
        .bind(&cron_config)
        .bind(&future)
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();

        // Run one tick
        TaskScheduler::tick(pool).await.unwrap();

        // Verify: a run was created for the due trigger
        let runs: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT id, trigger_id, idempotency_key FROM task_runs WHERE task_id = ?",
        )
        .bind(&task_id)
        .fetch_all(pool)
        .await
        .unwrap();

        assert_eq!(runs.len(), 1, "expected exactly 1 run to be created for the due trigger");
        assert_eq!(runs[0].1, trigger_id, "run should reference the due trigger");
        assert!(
            runs[0].2.starts_with(&trigger_id),
            "idempotency_key should start with trigger_id"
        );

        // Verify: the due trigger's next_fire_at was updated (should be in the future now)
        let updated_trigger: (Option<String>, Option<String>) = sqlx::query_as(
            "SELECT next_fire_at, last_fired_at FROM task_triggers WHERE id = ?",
        )
        .bind(&trigger_id)
        .fetch_one(pool)
        .await
        .unwrap();

        assert!(
            updated_trigger.0.is_some(),
            "next_fire_at should be set after tick"
        );
        let new_next = chrono::DateTime::parse_from_rfc3339(updated_trigger.0.as_ref().unwrap())
            .unwrap()
            .with_timezone(&Utc);
        assert!(
            new_next > now,
            "updated next_fire_at {} should be in the future (after {})",
            new_next,
            now
        );

        assert!(
            updated_trigger.1.is_some(),
            "last_fired_at should be set after tick"
        );

        // Verify: the future trigger was NOT fired (still has original next_fire_at)
        let future_trigger: (Option<String>,) = sqlx::query_as(
            "SELECT last_fired_at FROM task_triggers WHERE id = ?",
        )
        .bind(&trigger_id_future)
        .fetch_one(pool)
        .await
        .unwrap();

        assert!(
            future_trigger.0.is_none(),
            "future trigger should not have been fired"
        );
    }

    #[tokio::test]
    async fn scheduler_idempotency_prevents_duplicate_runs() {
        use chrono::Duration;

        let db = clawx_runtime::db::Database::in_memory().await.unwrap();
        let pool = &db.main;

        let now = Utc::now();
        let now_str = now.to_rfc3339();
        let agent_id = "00000000-0000-0000-0000-000000000002";

        sqlx::query(
            "INSERT INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'idle', '[]', ?, ?)",
        )
        .bind(agent_id)
        .bind("TestAgent2")
        .bind("assistant")
        .bind("default")
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();

        let task_id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO tasks (id, agent_id, name, goal, source_kind, lifecycle_status,
             default_max_steps, default_timeout_secs, notification_policy, suppression_state,
             created_at, updated_at)
             VALUES (?, ?, ?, ?, 'manual', 'active', 10, 1800, '{}', 'normal', ?, ?)",
        )
        .bind(&task_id)
        .bind(agent_id)
        .bind("Idempotency Task")
        .bind("Test goal")
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();

        let trigger_id = uuid::Uuid::new_v4().to_string();
        let past = (now - Duration::hours(1)).to_rfc3339();
        let cron_config = serde_json::json!({"cron": "0 * * * * *"}).to_string();

        sqlx::query(
            "INSERT INTO task_triggers (id, task_id, trigger_kind, trigger_config, status,
             next_fire_at, created_at, updated_at)
             VALUES (?, ?, 'time', ?, 'active', ?, ?, ?)",
        )
        .bind(&trigger_id)
        .bind(&task_id)
        .bind(&cron_config)
        .bind(&past)
        .bind(&now_str)
        .bind(&now_str)
        .execute(pool)
        .await
        .unwrap();

        // First tick creates a run
        TaskScheduler::tick(pool).await.unwrap();

        // Reset trigger's next_fire_at to past again to simulate re-fire
        sqlx::query("UPDATE task_triggers SET next_fire_at = ? WHERE id = ?")
            .bind(&past)
            .bind(&trigger_id)
            .execute(pool)
            .await
            .unwrap();

        // Second tick should skip because idempotency key exists
        TaskScheduler::tick(pool).await.unwrap();

        let count: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM task_runs WHERE task_id = ?")
                .bind(&task_id)
                .fetch_one(pool)
                .await
                .unwrap();

        // The first tick created 1 run. The second tick uses same idempotency key,
        // so it should skip. Count should be 1.
        assert_eq!(count.0, 1, "idempotency should prevent duplicate runs");
    }

    #[tokio::test]
    async fn scheduler_start_stop() {
        let db = clawx_runtime::db::Database::in_memory().await.unwrap();
        let scheduler = TaskScheduler::new(db.main.clone(), Duration::from_millis(50));

        assert!(!scheduler.is_running());

        let handle = scheduler.start();
        assert!(scheduler.is_running());

        // Let it run briefly
        tokio::time::sleep(Duration::from_millis(100)).await;

        scheduler.stop();
        assert!(!scheduler.is_running());

        // Wait for task to finish
        let _ = tokio::time::timeout(Duration::from_secs(2), handle).await;
    }

    use chrono::Timelike;

    // -----------------------------------------------------------------------
    // Event trigger tests
    // -----------------------------------------------------------------------

    /// Helper to set up a DB with an agent + task + event trigger.
    async fn setup_event_trigger(
        pool: &SqlitePool,
        agent_id: &str,
        task_id: &str,
        trigger_id: &str,
        event_kind_config: &str,
        trigger_status: &str,
    ) {
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "INSERT OR IGNORE INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, ?, ?, ?, 'idle', '[]', ?, ?)",
        )
        .bind(agent_id)
        .bind("EventAgent")
        .bind("assistant")
        .bind("default")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();

        sqlx::query(
            "INSERT OR IGNORE INTO tasks (id, agent_id, name, goal, source_kind, lifecycle_status,
             default_max_steps, default_timeout_secs, notification_policy, suppression_state,
             created_at, updated_at)
             VALUES (?, ?, ?, ?, 'manual', 'active', 10, 1800, '{}', 'normal', ?, ?)",
        )
        .bind(task_id)
        .bind(agent_id)
        .bind("Event Task")
        .bind("React to events")
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();

        let config = serde_json::json!({"event_kind": event_kind_config}).to_string();
        sqlx::query(
            "INSERT INTO task_triggers (id, task_id, trigger_kind, trigger_config, status,
             created_at, updated_at)
             VALUES (?, ?, 'event', ?, ?, ?, ?)",
        )
        .bind(trigger_id)
        .bind(task_id)
        .bind(&config)
        .bind(trigger_status)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn handle_event_creates_run_for_matching_trigger() {
        let db = clawx_runtime::db::Database::in_memory().await.unwrap();
        let pool = &db.main;

        let agent_id = "00000000-0000-0000-0000-000000000010";
        let task_id = uuid::Uuid::new_v4().to_string();
        let trigger_id = uuid::Uuid::new_v4().to_string();

        setup_event_trigger(pool, agent_id, &task_id, &trigger_id, "file_changed", "active").await;

        let runs = handle_event(pool, "file_changed", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(runs.len(), 1, "expected 1 run created for matching event trigger");

        // Verify the run exists in DB
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM task_runs WHERE task_id = ?")
            .bind(&task_id)
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(count.0, 1);
    }

    #[tokio::test]
    async fn handle_event_ignores_non_matching() {
        let db = clawx_runtime::db::Database::in_memory().await.unwrap();
        let pool = &db.main;

        let agent_id = "00000000-0000-0000-0000-000000000011";
        let task_id = uuid::Uuid::new_v4().to_string();
        let trigger_id = uuid::Uuid::new_v4().to_string();

        setup_event_trigger(pool, agent_id, &task_id, &trigger_id, "file_changed", "active").await;

        // Send a different event kind
        let runs = handle_event(pool, "network_change", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(runs.len(), 0, "non-matching event should create no runs");
    }

    #[tokio::test]
    async fn handle_event_multiple_triggers() {
        let db = clawx_runtime::db::Database::in_memory().await.unwrap();
        let pool = &db.main;

        let agent_id = "00000000-0000-0000-0000-000000000012";
        let task_id = uuid::Uuid::new_v4().to_string();
        let trigger_id_1 = uuid::Uuid::new_v4().to_string();
        let trigger_id_2 = uuid::Uuid::new_v4().to_string();

        setup_event_trigger(pool, agent_id, &task_id, &trigger_id_1, "disk_alert", "active").await;
        setup_event_trigger(pool, agent_id, &task_id, &trigger_id_2, "disk_alert", "active").await;

        let runs = handle_event(pool, "disk_alert", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(runs.len(), 2, "two matching triggers should create two runs");
    }

    #[tokio::test]
    async fn handle_event_paused_trigger_ignored() {
        let db = clawx_runtime::db::Database::in_memory().await.unwrap();
        let pool = &db.main;

        let agent_id = "00000000-0000-0000-0000-000000000013";
        let task_id = uuid::Uuid::new_v4().to_string();
        let trigger_id = uuid::Uuid::new_v4().to_string();

        // Trigger is paused
        setup_event_trigger(pool, agent_id, &task_id, &trigger_id, "file_changed", "paused").await;

        let runs = handle_event(pool, "file_changed", &serde_json::json!({}))
            .await
            .unwrap();

        assert_eq!(runs.len(), 0, "paused trigger should not fire");
    }

    #[test]
    fn matches_event_correct_match() {
        let config = serde_json::json!({"event_kind": "file_changed"});
        assert!(matches_event(&config, "file_changed"));
    }

    #[test]
    fn matches_event_no_match() {
        let config = serde_json::json!({"event_kind": "file_changed"});
        assert!(!matches_event(&config, "network_change"));
    }

    #[test]
    fn matches_event_missing_key() {
        let config = serde_json::json!({"cron": "0 * * * * *"});
        assert!(!matches_event(&config, "file_changed"));
    }

    #[test]
    fn matches_event_null_value() {
        let config = serde_json::json!({"event_kind": null});
        assert!(!matches_event(&config, "file_changed"));
    }
}
