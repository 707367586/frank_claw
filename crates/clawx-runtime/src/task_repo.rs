//! Task registry repository -- thin layer over SQLite task tables.
//!
//! Implements `TaskRegistryPort` for proactive task CRUD, trigger management,
//! run tracking, and feedback recording.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use clawx_types::autonomy::*;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::{AgentId, TaskId};
use clawx_types::pagination::{PagedResult, Pagination};
use clawx_types::traits::{RunUpdate, TaskRegistryPort, TaskUpdate, TriggerUpdate};
use sqlx::SqlitePool;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Row structs
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct TaskRow {
    id: String,
    agent_id: String,
    name: String,
    goal: String,
    source_kind: String,
    lifecycle_status: String,
    default_max_steps: i64,
    default_timeout_secs: i64,
    notification_policy: String,
    suppression_state: String,
    last_run_at: Option<String>,
    next_run_at: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<TaskRow> for Task {
    type Error = ClawxError;

    fn try_from(row: TaskRow) -> Result<Self> {
        Ok(Task {
            id: TaskId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid task id: {}", e)))?,
            agent_id: AgentId::from_str(&row.agent_id)
                .map_err(|e| ClawxError::Database(format!("invalid agent id: {}", e)))?,
            name: row.name,
            goal: row.goal,
            source_kind: TaskSourceKind::from_str(&row.source_kind)
                .map_err(|e| ClawxError::Database(format!("invalid source_kind: {}", e)))?,
            lifecycle_status: TaskLifecycleStatus::from_str(&row.lifecycle_status)
                .map_err(|e| ClawxError::Database(format!("invalid lifecycle_status: {}", e)))?,
            default_max_steps: row.default_max_steps as u32,
            default_timeout_secs: row.default_timeout_secs as u32,
            notification_policy: serde_json::from_str(&row.notification_policy)
                .map_err(|e| ClawxError::Database(format!("invalid notification_policy: {}", e)))?,
            suppression_state: SuppressionState::from_str(&row.suppression_state)
                .map_err(|e| ClawxError::Database(format!("invalid suppression_state: {}", e)))?,
            last_run_at: parse_optional_datetime(&row.last_run_at, "last_run_at")?,
            next_run_at: parse_optional_datetime(&row.next_run_at, "next_run_at")?,
            created_at: parse_datetime(&row.created_at, "created_at")?,
            updated_at: parse_datetime(&row.updated_at, "updated_at")?,
        })
    }
}

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

impl TryFrom<TriggerRow> for Trigger {
    type Error = ClawxError;

    fn try_from(row: TriggerRow) -> Result<Self> {
        Ok(Trigger {
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
            next_fire_at: parse_optional_datetime(&row.next_fire_at, "next_fire_at")?,
            last_fired_at: parse_optional_datetime(&row.last_fired_at, "last_fired_at")?,
            created_at: parse_datetime(&row.created_at, "created_at")?,
            updated_at: parse_datetime(&row.updated_at, "updated_at")?,
        })
    }
}

#[derive(Debug, sqlx::FromRow)]
struct RunRow {
    id: String,
    task_id: String,
    trigger_id: Option<String>,
    idempotency_key: String,
    run_status: String,
    attempt: i64,
    lease_owner: Option<String>,
    lease_expires_at: Option<String>,
    checkpoint: String,
    tokens_used: i64,
    steps_count: i64,
    result_summary: Option<String>,
    failure_reason: Option<String>,
    feedback_kind: Option<String>,
    feedback_reason: Option<String>,
    notification_status: String,
    triggered_at: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    created_at: String,
}

impl TryFrom<RunRow> for Run {
    type Error = ClawxError;

    fn try_from(row: RunRow) -> Result<Self> {
        Ok(Run {
            id: RunId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid run id: {}", e)))?,
            task_id: TaskId::from_str(&row.task_id)
                .map_err(|e| ClawxError::Database(format!("invalid task id: {}", e)))?,
            trigger_id: row
                .trigger_id
                .map(|s| {
                    TriggerId::from_str(&s)
                        .map_err(|e| ClawxError::Database(format!("invalid trigger id: {}", e)))
                })
                .transpose()?,
            idempotency_key: row.idempotency_key,
            run_status: RunStatus::from_str(&row.run_status)
                .map_err(|e| ClawxError::Database(format!("invalid run_status: {}", e)))?,
            attempt: row.attempt as u32,
            lease_owner: row.lease_owner,
            lease_expires_at: parse_optional_datetime(&row.lease_expires_at, "lease_expires_at")?,
            checkpoint: serde_json::from_str(&row.checkpoint)
                .map_err(|e| ClawxError::Database(format!("invalid checkpoint: {}", e)))?,
            tokens_used: row.tokens_used as u64,
            steps_count: row.steps_count as u32,
            result_summary: row.result_summary,
            failure_reason: row.failure_reason,
            feedback_kind: row
                .feedback_kind
                .map(|s| {
                    FeedbackKind::from_str(&s)
                        .map_err(|e| ClawxError::Database(format!("invalid feedback_kind: {}", e)))
                })
                .transpose()?,
            feedback_reason: row.feedback_reason,
            notification_status: NotificationStatus::from_str(&row.notification_status)
                .map_err(|e| {
                    ClawxError::Database(format!("invalid notification_status: {}", e))
                })?,
            triggered_at: parse_datetime(&row.triggered_at, "triggered_at")?,
            started_at: parse_optional_datetime(&row.started_at, "started_at")?,
            finished_at: parse_optional_datetime(&row.finished_at, "finished_at")?,
            created_at: parse_datetime(&row.created_at, "created_at")?,
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_datetime(s: &str, field: &str) -> Result<DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|e| ClawxError::Database(format!("invalid {}: {}", field, e)))
}

fn parse_optional_datetime(
    s: &Option<String>,
    field: &str,
) -> Result<Option<DateTime<Utc>>> {
    s.as_ref()
        .map(|s| parse_datetime(s, field))
        .transpose()
}

// ---------------------------------------------------------------------------
// SqliteTaskRegistry
// ---------------------------------------------------------------------------

/// Task registry backed by SQLite.
pub struct SqliteTaskRegistry {
    pool: SqlitePool,
}

impl SqliteTaskRegistry {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl TaskRegistryPort for SqliteTaskRegistry {
    // -- Task CRUD --

    async fn create_task(&self, task: Task) -> Result<TaskId> {
        let id = task.id.to_string();
        let agent_id = task.agent_id.to_string();
        let source_kind = task.source_kind.to_string();
        let lifecycle_status = task.lifecycle_status.to_string();
        let notification_policy = serde_json::to_string(&task.notification_policy)
            .map_err(|e| ClawxError::Task(format!("serialize notification_policy: {}", e)))?;
        let suppression_state = task.suppression_state.to_string();
        let last_run_at = task.last_run_at.map(|dt| dt.to_rfc3339());
        let next_run_at = task.next_run_at.map(|dt| dt.to_rfc3339());
        let created_at = task.created_at.to_rfc3339();
        let updated_at = task.updated_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO tasks (id, agent_id, name, goal, source_kind, lifecycle_status,
             default_max_steps, default_timeout_secs, notification_policy, suppression_state,
             last_run_at, next_run_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&agent_id)
        .bind(&task.name)
        .bind(&task.goal)
        .bind(&source_kind)
        .bind(&lifecycle_status)
        .bind(task.default_max_steps as i64)
        .bind(task.default_timeout_secs as i64)
        .bind(&notification_policy)
        .bind(&suppression_state)
        .bind(&last_run_at)
        .bind(&next_run_at)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("create task: {}", e)))?;

        Ok(task.id)
    }

    async fn get_task(&self, id: TaskId) -> Result<Option<Task>> {
        let row: Option<TaskRow> = sqlx::query_as("SELECT * FROM tasks WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(format!("get task: {}", e)))?;

        row.map(Task::try_from).transpose()
    }

    async fn list_tasks(
        &self,
        agent_id: Option<AgentId>,
        pagination: Pagination,
    ) -> Result<PagedResult<Task>> {
        let offset = (pagination.page.saturating_sub(1)) * pagination.per_page;

        let (total, rows): (i64, Vec<TaskRow>) = if let Some(aid) = agent_id {
            let aid_str = aid.to_string();
            let count: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM tasks WHERE agent_id = ?",
            )
            .bind(&aid_str)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(format!("count tasks: {}", e)))?;

            let rows: Vec<TaskRow> = sqlx::query_as(
                "SELECT * FROM tasks WHERE agent_id = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(&aid_str)
            .bind(pagination.per_page as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(format!("list tasks: {}", e)))?;

            (count.0, rows)
        } else {
            let count: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM tasks")
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|e| ClawxError::Database(format!("count tasks: {}", e)))?;

            let rows: Vec<TaskRow> = sqlx::query_as(
                "SELECT * FROM tasks ORDER BY created_at DESC LIMIT ? OFFSET ?",
            )
            .bind(pagination.per_page as i64)
            .bind(offset as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(format!("list tasks: {}", e)))?;

            (count.0, rows)
        };

        let items: Vec<Task> = rows.into_iter().map(Task::try_from).collect::<Result<_>>()?;

        Ok(PagedResult {
            items,
            total: total as u64,
            page: pagination.page,
            per_page: pagination.per_page,
        })
    }

    async fn update_task(&self, id: TaskId, update: TaskUpdate) -> Result<()> {
        let existing = self
            .get_task(id)
            .await?
            .ok_or_else(|| ClawxError::NotFound {
                entity: "task".into(),
                id: id.to_string(),
            })?;

        let name = update.name.as_deref().unwrap_or(&existing.name);
        let goal = update.goal.as_deref().unwrap_or(&existing.goal);
        let notification_policy = match &update.notification_policy {
            Some(v) => serde_json::to_string(v)
                .map_err(|e| ClawxError::Task(format!("serialize notification_policy: {}", e)))?,
            None => serde_json::to_string(&existing.notification_policy)
                .map_err(|e| ClawxError::Task(format!("serialize notification_policy: {}", e)))?,
        };
        let max_steps = update.default_max_steps.unwrap_or(existing.default_max_steps) as i64;
        let timeout = update
            .default_timeout_secs
            .unwrap_or(existing.default_timeout_secs) as i64;
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE tasks SET name = ?, goal = ?, notification_policy = ?,
             default_max_steps = ?, default_timeout_secs = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(name)
        .bind(goal)
        .bind(&notification_policy)
        .bind(max_steps)
        .bind(timeout)
        .bind(&now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("update task: {}", e)))?;

        Ok(())
    }

    async fn delete_task(&self, id: TaskId) -> Result<()> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(format!("delete task: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(ClawxError::NotFound {
                entity: "task".into(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    async fn update_lifecycle(&self, id: TaskId, status: TaskLifecycleStatus) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let result = sqlx::query(
            "UPDATE tasks SET lifecycle_status = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status.to_string())
        .bind(&now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("update lifecycle: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(ClawxError::NotFound {
                entity: "task".into(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    // -- Trigger CRUD --

    async fn add_trigger(&self, trigger: Trigger) -> Result<TriggerId> {
        let id = trigger.id.to_string();
        let task_id = trigger.task_id.to_string();
        let trigger_kind = trigger.trigger_kind.to_string();
        let trigger_config = serde_json::to_string(&trigger.trigger_config)
            .map_err(|e| ClawxError::Task(format!("serialize trigger_config: {}", e)))?;
        let status = trigger.status.to_string();
        let next_fire_at = trigger.next_fire_at.map(|dt| dt.to_rfc3339());
        let last_fired_at = trigger.last_fired_at.map(|dt| dt.to_rfc3339());
        let created_at = trigger.created_at.to_rfc3339();
        let updated_at = trigger.updated_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO task_triggers (id, task_id, trigger_kind, trigger_config, status,
             next_fire_at, last_fired_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&task_id)
        .bind(&trigger_kind)
        .bind(&trigger_config)
        .bind(&status)
        .bind(&next_fire_at)
        .bind(&last_fired_at)
        .bind(&created_at)
        .bind(&updated_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("add trigger: {}", e)))?;

        Ok(trigger.id)
    }

    async fn get_trigger(&self, id: TriggerId) -> Result<Option<Trigger>> {
        let row: Option<TriggerRow> =
            sqlx::query_as("SELECT * FROM task_triggers WHERE id = ?")
                .bind(id.to_string())
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| ClawxError::Database(format!("get trigger: {}", e)))?;

        row.map(Trigger::try_from).transpose()
    }

    async fn list_triggers(&self, task_id: TaskId) -> Result<Vec<Trigger>> {
        let rows: Vec<TriggerRow> = sqlx::query_as(
            "SELECT * FROM task_triggers WHERE task_id = ? ORDER BY created_at DESC",
        )
        .bind(task_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("list triggers: {}", e)))?;

        rows.into_iter().map(Trigger::try_from).collect()
    }

    async fn update_trigger(&self, id: TriggerId, update: TriggerUpdate) -> Result<()> {
        let existing = self
            .get_trigger(id)
            .await?
            .ok_or_else(|| ClawxError::NotFound {
                entity: "trigger".into(),
                id: id.to_string(),
            })?;

        let trigger_config = match &update.trigger_config {
            Some(v) => serde_json::to_string(v)
                .map_err(|e| ClawxError::Task(format!("serialize trigger_config: {}", e)))?,
            None => serde_json::to_string(&existing.trigger_config)
                .map_err(|e| ClawxError::Task(format!("serialize trigger_config: {}", e)))?,
        };
        let status = update
            .status
            .unwrap_or(existing.status)
            .to_string();
        let next_fire_at = match update.next_fire_at {
            Some(dt) => Some(dt.to_rfc3339()),
            None => existing.next_fire_at.map(|dt| dt.to_rfc3339()),
        };
        let last_fired_at = match update.last_fired_at {
            Some(dt) => Some(dt.to_rfc3339()),
            None => existing.last_fired_at.map(|dt| dt.to_rfc3339()),
        };
        let now = Utc::now().to_rfc3339();

        sqlx::query(
            "UPDATE task_triggers SET trigger_config = ?, status = ?, next_fire_at = ?, last_fired_at = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&trigger_config)
        .bind(&status)
        .bind(&next_fire_at)
        .bind(&last_fired_at)
        .bind(&now)
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("update trigger: {}", e)))?;

        Ok(())
    }

    async fn delete_trigger(&self, id: TriggerId) -> Result<()> {
        let result = sqlx::query("DELETE FROM task_triggers WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(format!("delete trigger: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(ClawxError::NotFound {
                entity: "trigger".into(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    async fn get_due_triggers(&self, now: DateTime<Utc>) -> Result<Vec<Trigger>> {
        let now_str = now.to_rfc3339();
        let rows: Vec<TriggerRow> = sqlx::query_as(
            "SELECT * FROM task_triggers WHERE status = 'active' AND next_fire_at IS NOT NULL AND next_fire_at <= ?
             ORDER BY next_fire_at ASC",
        )
        .bind(&now_str)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("get due triggers: {}", e)))?;

        rows.into_iter().map(Trigger::try_from).collect()
    }

    // -- Run CRUD --

    async fn create_run(&self, run: Run) -> Result<RunId> {
        let id = run.id.to_string();
        let task_id = run.task_id.to_string();
        let trigger_id = run.trigger_id.map(|t| t.to_string());
        let run_status = run.run_status.to_string();
        let checkpoint = serde_json::to_string(&run.checkpoint)
            .map_err(|e| ClawxError::Task(format!("serialize checkpoint: {}", e)))?;
        let feedback_kind = run.feedback_kind.map(|f| f.to_string());
        let notification_status = run.notification_status.to_string();
        let triggered_at = run.triggered_at.to_rfc3339();
        let started_at = run.started_at.map(|dt| dt.to_rfc3339());
        let finished_at = run.finished_at.map(|dt| dt.to_rfc3339());
        let created_at = run.created_at.to_rfc3339();
        let lease_expires_at = run.lease_expires_at.map(|dt| dt.to_rfc3339());

        sqlx::query(
            "INSERT INTO task_runs (id, task_id, trigger_id, idempotency_key, run_status,
             attempt, lease_owner, lease_expires_at, checkpoint, tokens_used, steps_count,
             result_summary, failure_reason, feedback_kind, feedback_reason,
             notification_status, triggered_at, started_at, finished_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&task_id)
        .bind(&trigger_id)
        .bind(&run.idempotency_key)
        .bind(&run_status)
        .bind(run.attempt as i64)
        .bind(&run.lease_owner)
        .bind(&lease_expires_at)
        .bind(&checkpoint)
        .bind(run.tokens_used as i64)
        .bind(run.steps_count as i64)
        .bind(&run.result_summary)
        .bind(&run.failure_reason)
        .bind(&feedback_kind)
        .bind(&run.feedback_reason)
        .bind(&notification_status)
        .bind(&triggered_at)
        .bind(&started_at)
        .bind(&finished_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("create run: {}", e)))?;

        Ok(run.id)
    }

    async fn get_run(&self, id: RunId) -> Result<Option<Run>> {
        let row: Option<RunRow> = sqlx::query_as("SELECT * FROM task_runs WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| ClawxError::Database(format!("get run: {}", e)))?;

        row.map(Run::try_from).transpose()
    }

    async fn list_runs(
        &self,
        task_id: TaskId,
        pagination: Pagination,
    ) -> Result<PagedResult<Run>> {
        let offset = (pagination.page.saturating_sub(1)) * pagination.per_page;
        let task_id_str = task_id.to_string();

        let count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM task_runs WHERE task_id = ?",
        )
        .bind(&task_id_str)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("count runs: {}", e)))?;

        let rows: Vec<RunRow> = sqlx::query_as(
            "SELECT * FROM task_runs WHERE task_id = ? ORDER BY triggered_at DESC LIMIT ? OFFSET ?",
        )
        .bind(&task_id_str)
        .bind(pagination.per_page as i64)
        .bind(offset as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("list runs: {}", e)))?;

        let items: Vec<Run> = rows.into_iter().map(Run::try_from).collect::<Result<_>>()?;

        Ok(PagedResult {
            items,
            total: count.0 as u64,
            page: pagination.page,
            per_page: pagination.per_page,
        })
    }

    async fn update_run(&self, id: RunId, update: RunUpdate) -> Result<()> {
        let existing = self
            .get_run(id)
            .await?
            .ok_or_else(|| ClawxError::NotFound {
                entity: "run".into(),
                id: id.to_string(),
            })?;

        let run_status = update
            .run_status
            .unwrap_or(existing.run_status)
            .to_string();
        let checkpoint = match &update.checkpoint {
            Some(v) => serde_json::to_string(v)
                .map_err(|e| ClawxError::Task(format!("serialize checkpoint: {}", e)))?,
            None => serde_json::to_string(&existing.checkpoint)
                .map_err(|e| ClawxError::Task(format!("serialize checkpoint: {}", e)))?,
        };
        let tokens_used = update.tokens_used.unwrap_or(existing.tokens_used) as i64;
        let steps_count = update.steps_count.unwrap_or(existing.steps_count) as i64;
        let result_summary = update.result_summary.as_deref().or(existing.result_summary.as_deref());
        let failure_reason = update.failure_reason.as_deref().or(existing.failure_reason.as_deref());
        let notification_status = update
            .notification_status
            .unwrap_or(existing.notification_status)
            .to_string();
        let started_at = match update.started_at {
            Some(dt) => Some(dt.to_rfc3339()),
            None => existing.started_at.map(|dt| dt.to_rfc3339()),
        };
        let finished_at = match update.finished_at {
            Some(dt) => Some(dt.to_rfc3339()),
            None => existing.finished_at.map(|dt| dt.to_rfc3339()),
        };

        sqlx::query(
            "UPDATE task_runs SET run_status = ?, checkpoint = ?, tokens_used = ?,
             steps_count = ?, result_summary = ?, failure_reason = ?,
             notification_status = ?, started_at = ?, finished_at = ?
             WHERE id = ?",
        )
        .bind(&run_status)
        .bind(&checkpoint)
        .bind(tokens_used)
        .bind(steps_count)
        .bind(result_summary)
        .bind(failure_reason)
        .bind(&notification_status)
        .bind(&started_at)
        .bind(&finished_at)
        .bind(id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("update run: {}", e)))?;

        Ok(())
    }

    async fn get_incomplete_runs(&self) -> Result<Vec<Run>> {
        let rows: Vec<RunRow> = sqlx::query_as(
            "SELECT * FROM task_runs WHERE run_status IN ('queued', 'planning', 'running', 'waiting_confirmation')
             ORDER BY triggered_at ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("get incomplete runs: {}", e)))?;

        rows.into_iter().map(Run::try_from).collect()
    }

    // -- Feedback --

    async fn record_feedback(
        &self,
        run_id: RunId,
        kind: FeedbackKind,
        reason: Option<String>,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE task_runs SET feedback_kind = ?, feedback_reason = ? WHERE id = ?",
        )
        .bind(kind.to_string())
        .bind(reason.as_deref())
        .bind(run_id.to_string())
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("record feedback: {}", e)))?;

        if result.rows_affected() == 0 {
            return Err(ClawxError::NotFound {
                entity: "run".into(),
                id: run_id.to_string(),
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Free-function convenience API (wraps SqliteTaskRegistry)
// ---------------------------------------------------------------------------

pub async fn create_task(pool: &SqlitePool, task: &Task) -> Result<TaskId> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.create_task(task.clone()).await
}

pub async fn get_task(pool: &SqlitePool, id: TaskId) -> Result<Option<Task>> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.get_task(id).await
}

pub async fn list_tasks(pool: &SqlitePool, agent_id: Option<AgentId>) -> Result<Vec<Task>> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    let result = reg.list_tasks(agent_id, Pagination::default()).await?;
    Ok(result.items)
}

pub async fn update_task(
    pool: &SqlitePool,
    id: TaskId,
    name: Option<&str>,
    goal: Option<&str>,
    notification_policy: Option<&serde_json::Value>,
    default_max_steps: Option<u32>,
    default_timeout_secs: Option<u32>,
) -> Result<()> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    let update = TaskUpdate {
        name: name.map(String::from),
        goal: goal.map(String::from),
        notification_policy: notification_policy.cloned(),
        default_max_steps,
        default_timeout_secs,
    };
    reg.update_task(id, update).await
}

pub async fn delete_task(pool: &SqlitePool, id: TaskId) -> Result<()> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.delete_task(id).await
}

pub async fn update_lifecycle(pool: &SqlitePool, id: TaskId, status: &str) -> Result<()> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    let lifecycle = TaskLifecycleStatus::from_str(status)
        .map_err(|e| ClawxError::Validation(e))?;
    reg.update_lifecycle(id, lifecycle).await
}

pub async fn create_trigger(pool: &SqlitePool, trigger: &Trigger) -> Result<TriggerId> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.add_trigger(trigger.clone()).await
}

pub async fn get_trigger(pool: &SqlitePool, id: TriggerId) -> Result<Option<Trigger>> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.get_trigger(id).await
}

pub async fn list_triggers(pool: &SqlitePool, task_id: TaskId) -> Result<Vec<Trigger>> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.list_triggers(task_id).await
}

pub async fn update_trigger(
    pool: &SqlitePool,
    id: TriggerId,
    trigger_config: Option<&serde_json::Value>,
    status: Option<&str>,
    next_fire_at: Option<&str>,
) -> Result<()> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    let update = TriggerUpdate {
        trigger_config: trigger_config.cloned(),
        status: status.map(|s| TriggerStatus::from_str(s).ok()).flatten(),
        next_fire_at: next_fire_at
            .map(|s| DateTime::parse_from_rfc3339(s).ok().map(|d| d.with_timezone(&Utc)))
            .flatten(),
        last_fired_at: None,
    };
    reg.update_trigger(id, update).await
}

pub async fn delete_trigger(pool: &SqlitePool, id: TriggerId) -> Result<()> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.delete_trigger(id).await
}

pub async fn list_runs(pool: &SqlitePool, task_id: TaskId) -> Result<Vec<Run>> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    let result = reg.list_runs(task_id, Pagination::default()).await?;
    Ok(result.items)
}

pub async fn get_run(pool: &SqlitePool, id: RunId) -> Result<Option<Run>> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.get_run(id).await
}

pub async fn create_run(pool: &SqlitePool, run: &Run) -> Result<RunId> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    reg.create_run(run.clone()).await
}

/// Atomically transition a run's status, returning the updated Run.
/// Returns `None` if the run doesn't exist or its current status doesn't match
/// any of the `expected_statuses`. This prevents TOCTOU race conditions.
pub async fn transition_run_status(
    pool: &SqlitePool,
    id: RunId,
    expected_statuses: &[RunStatus],
    update: RunUpdate,
) -> Result<Option<Run>> {
    let status_strings: Vec<String> = expected_statuses.iter().map(|s| s.to_string()).collect();
    let in_clause = status_strings.iter().map(|_| "?").collect::<Vec<_>>().join(", ");

    let new_status = update.run_status.map(|s| s.to_string());
    let finished_at = update.finished_at.map(|dt| dt.to_rfc3339());

    let query_str = format!(
        "UPDATE task_runs SET run_status = COALESCE(?, run_status), \
         finished_at = COALESCE(?, finished_at) \
         WHERE id = ? AND run_status IN ({})",
        in_clause
    );
    let mut query = sqlx::query(&query_str)
        .bind(&new_status)
        .bind(&finished_at)
        .bind(id.to_string());
    for status in &status_strings {
        query = query.bind(status);
    }

    let result = query
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("transition run status: {}", e)))?;

    if result.rows_affected() == 0 {
        return Ok(None);
    }

    get_run(pool, id).await
}

pub async fn record_feedback(
    pool: &SqlitePool,
    run_id: RunId,
    kind: &str,
    reason: Option<&str>,
) -> Result<()> {
    let reg = SqliteTaskRegistry::new(pool.clone());
    let feedback = FeedbackKind::from_str(kind)
        .map_err(|e| ClawxError::Validation(e))?;
    reg.record_feedback(run_id, feedback, reason.map(String::from)).await
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    async fn setup() -> (Database, SqliteTaskRegistry) {
        let db = Database::in_memory().await.unwrap();
        // Insert a dummy agent so FK constraints pass
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

    const TEST_AGENT_ID: &str = "00000000-0000-0000-0000-000000000001";

    fn make_task(name: &str) -> Task {
        let now = Utc::now();
        Task {
            id: TaskId::new(),
            agent_id: AgentId::from_str(TEST_AGENT_ID).unwrap(),
            name: name.to_string(),
            goal: format!("Goal for {}", name),
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

    fn make_trigger(task_id: TaskId, fire_at: Option<DateTime<Utc>>) -> Trigger {
        let now = Utc::now();
        Trigger {
            id: TriggerId::new(),
            task_id,
            trigger_kind: TriggerKind::Time,
            trigger_config: serde_json::json!({"cron": "0 * * * *"}),
            status: TriggerStatus::Active,
            next_fire_at: fire_at,
            last_fired_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_run(task_id: TaskId, key: &str) -> Run {
        let now = Utc::now();
        Run {
            id: RunId::new(),
            task_id,
            trigger_id: None,
            idempotency_key: key.to_string(),
            run_status: RunStatus::Queued,
            attempt: 1,
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
    // Task tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn create_and_get_task() {
        let (_db, reg) = setup().await;
        let task = make_task("Daily Report");
        let id = reg.create_task(task.clone()).await.unwrap();
        assert_eq!(id, task.id);

        let fetched = reg.get_task(id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Daily Report");
        assert_eq!(fetched.goal, "Goal for Daily Report");
        assert_eq!(fetched.source_kind, TaskSourceKind::Manual);
        assert_eq!(fetched.lifecycle_status, TaskLifecycleStatus::Active);
        assert_eq!(fetched.default_max_steps, 10);
    }

    #[tokio::test]
    async fn get_nonexistent_task_returns_none() {
        let (_db, reg) = setup().await;
        let result = reg.get_task(TaskId::new()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_tasks_with_pagination() {
        let (_db, reg) = setup().await;
        for i in 0..5 {
            reg.create_task(make_task(&format!("Task {}", i)))
                .await
                .unwrap();
        }

        let page = reg
            .list_tasks(None, Pagination { page: 1, per_page: 3 })
            .await
            .unwrap();
        assert_eq!(page.items.len(), 3);
        assert_eq!(page.total, 5);

        // Filter by agent_id
        let page = reg
            .list_tasks(
                Some(AgentId::from_str(TEST_AGENT_ID).unwrap()),
                Pagination { page: 1, per_page: 20 },
            )
            .await
            .unwrap();
        assert_eq!(page.total, 5);
    }

    #[tokio::test]
    async fn update_task_partial() {
        let (_db, reg) = setup().await;
        let task = make_task("Original");
        let id = reg.create_task(task).await.unwrap();

        reg.update_task(
            id,
            TaskUpdate {
                name: Some("Renamed".to_string()),
                default_max_steps: Some(42),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let fetched = reg.get_task(id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Renamed");
        assert_eq!(fetched.default_max_steps, 42);
        // goal unchanged
        assert_eq!(fetched.goal, "Goal for Original");
    }

    #[tokio::test]
    async fn delete_task_removes_it() {
        let (_db, reg) = setup().await;
        let task = make_task("ToDelete");
        let id = reg.create_task(task).await.unwrap();

        reg.delete_task(id).await.unwrap();
        assert!(reg.get_task(id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_task_returns_not_found() {
        let (_db, reg) = setup().await;
        let result = reg.delete_task(TaskId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn update_lifecycle_transitions() {
        let (_db, reg) = setup().await;
        let task = make_task("LifecycleTask");
        let id = reg.create_task(task).await.unwrap();

        // active -> paused
        reg.update_lifecycle(id, TaskLifecycleStatus::Paused)
            .await
            .unwrap();
        let fetched = reg.get_task(id).await.unwrap().unwrap();
        assert_eq!(fetched.lifecycle_status, TaskLifecycleStatus::Paused);

        // paused -> archived
        reg.update_lifecycle(id, TaskLifecycleStatus::Archived)
            .await
            .unwrap();
        let fetched = reg.get_task(id).await.unwrap().unwrap();
        assert_eq!(fetched.lifecycle_status, TaskLifecycleStatus::Archived);
    }

    #[tokio::test]
    async fn update_lifecycle_nonexistent_returns_not_found() {
        let (_db, reg) = setup().await;
        let result = reg
            .update_lifecycle(TaskId::new(), TaskLifecycleStatus::Paused)
            .await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    // -----------------------------------------------------------------------
    // Trigger tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn create_and_get_trigger() {
        let (_db, reg) = setup().await;
        let task = make_task("TriggerTask");
        let task_id = reg.create_task(task).await.unwrap();

        let trigger = make_trigger(task_id, None);
        let tid = reg.add_trigger(trigger.clone()).await.unwrap();
        assert_eq!(tid, trigger.id);

        let fetched = reg.get_trigger(tid).await.unwrap().unwrap();
        assert_eq!(fetched.trigger_kind, TriggerKind::Time);
        assert_eq!(fetched.status, TriggerStatus::Active);
    }

    #[tokio::test]
    async fn list_triggers_for_task() {
        let (_db, reg) = setup().await;
        let task = make_task("TriggerListTask");
        let task_id = reg.create_task(task).await.unwrap();

        for _ in 0..3 {
            reg.add_trigger(make_trigger(task_id, None)).await.unwrap();
        }

        let triggers = reg.list_triggers(task_id).await.unwrap();
        assert_eq!(triggers.len(), 3);
    }

    #[tokio::test]
    async fn get_due_triggers_filters_correctly() {
        let (_db, reg) = setup().await;
        let task = make_task("DueTask");
        let task_id = reg.create_task(task).await.unwrap();

        let past = Utc::now() - chrono::Duration::hours(1);
        let future = Utc::now() + chrono::Duration::hours(1);

        reg.add_trigger(make_trigger(task_id, Some(past)))
            .await
            .unwrap();
        reg.add_trigger(make_trigger(task_id, Some(future)))
            .await
            .unwrap();
        // no fire_at trigger
        reg.add_trigger(make_trigger(task_id, None)).await.unwrap();

        let due = reg.get_due_triggers(Utc::now()).await.unwrap();
        assert_eq!(due.len(), 1);
        // The past trigger is due
        assert!(due[0].next_fire_at.unwrap() < Utc::now());
    }

    #[tokio::test]
    async fn update_trigger_partial() {
        let (_db, reg) = setup().await;
        let task = make_task("UpdateTriggerTask");
        let task_id = reg.create_task(task).await.unwrap();
        let trigger = make_trigger(task_id, None);
        let tid = reg.add_trigger(trigger).await.unwrap();

        reg.update_trigger(
            tid,
            TriggerUpdate {
                status: Some(TriggerStatus::Paused),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let fetched = reg.get_trigger(tid).await.unwrap().unwrap();
        assert_eq!(fetched.status, TriggerStatus::Paused);
    }

    // -----------------------------------------------------------------------
    // Run tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn create_and_get_run() {
        let (_db, reg) = setup().await;
        let task = make_task("RunTask");
        let task_id = reg.create_task(task).await.unwrap();

        let run = make_run(task_id, "key-1");
        let rid = reg.create_run(run.clone()).await.unwrap();
        assert_eq!(rid, run.id);

        let fetched = reg.get_run(rid).await.unwrap().unwrap();
        assert_eq!(fetched.run_status, RunStatus::Queued);
        assert_eq!(fetched.idempotency_key, "key-1");
    }

    #[tokio::test]
    async fn list_runs_with_pagination() {
        let (_db, reg) = setup().await;
        let task = make_task("RunListTask");
        let task_id = reg.create_task(task).await.unwrap();

        for i in 0..4 {
            reg.create_run(make_run(task_id, &format!("key-{}", i)))
                .await
                .unwrap();
        }

        let page = reg
            .list_runs(task_id, Pagination { page: 1, per_page: 2 })
            .await
            .unwrap();
        assert_eq!(page.items.len(), 2);
        assert_eq!(page.total, 4);
    }

    #[tokio::test]
    async fn update_run_status_and_fields() {
        let (_db, reg) = setup().await;
        let task = make_task("UpdateRunTask");
        let task_id = reg.create_task(task).await.unwrap();
        let run = make_run(task_id, "key-update");
        let rid = reg.create_run(run).await.unwrap();

        let now = Utc::now();
        reg.update_run(
            rid,
            RunUpdate {
                run_status: Some(RunStatus::Running),
                tokens_used: Some(500),
                steps_count: Some(3),
                started_at: Some(now),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let fetched = reg.get_run(rid).await.unwrap().unwrap();
        assert_eq!(fetched.run_status, RunStatus::Running);
        assert_eq!(fetched.tokens_used, 500);
        assert_eq!(fetched.steps_count, 3);
        assert!(fetched.started_at.is_some());
    }

    #[tokio::test]
    async fn get_incomplete_runs_returns_active_only() {
        let (_db, reg) = setup().await;
        let task = make_task("IncompleteRunTask");
        let task_id = reg.create_task(task).await.unwrap();

        // Create queued run
        let run1 = make_run(task_id, "key-incomplete-1");
        let r1 = reg.create_run(run1).await.unwrap();

        // Create completed run
        let run2 = make_run(task_id, "key-incomplete-2");
        let r2 = reg.create_run(run2).await.unwrap();
        reg.update_run(
            r2,
            RunUpdate {
                run_status: Some(RunStatus::Completed),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        // Create running run
        let run3 = make_run(task_id, "key-incomplete-3");
        let r3 = reg.create_run(run3).await.unwrap();
        reg.update_run(
            r3,
            RunUpdate {
                run_status: Some(RunStatus::Running),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        let incomplete = reg.get_incomplete_runs().await.unwrap();
        assert_eq!(incomplete.len(), 2); // queued + running
        let ids: Vec<RunId> = incomplete.iter().map(|r| r.id).collect();
        assert!(ids.contains(&r1));
        assert!(ids.contains(&r3));
        assert!(!ids.contains(&r2));
    }

    // -----------------------------------------------------------------------
    // Feedback test
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn record_feedback_on_run() {
        let (_db, reg) = setup().await;
        let task = make_task("FeedbackTask");
        let task_id = reg.create_task(task).await.unwrap();
        let run = make_run(task_id, "key-fb");
        let rid = reg.create_run(run).await.unwrap();

        reg.record_feedback(rid, FeedbackKind::Accepted, Some("Looks good".into()))
            .await
            .unwrap();

        let fetched = reg.get_run(rid).await.unwrap().unwrap();
        assert_eq!(fetched.feedback_kind, Some(FeedbackKind::Accepted));
        assert_eq!(fetched.feedback_reason.as_deref(), Some("Looks good"));
    }

    #[tokio::test]
    async fn record_feedback_nonexistent_run_returns_not_found() {
        let (_db, reg) = setup().await;
        let result = reg
            .record_feedback(RunId::new(), FeedbackKind::Rejected, None)
            .await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }
}
