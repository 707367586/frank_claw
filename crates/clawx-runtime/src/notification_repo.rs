//! Notification delivery repository and adapters.
//!
//! Records notification delivery attempts and provides adapters for
//! desktop notifications and file-based notifications.

use async_trait::async_trait;
use chrono::Utc;
use clawx_types::autonomy::*;
use clawx_types::error::{ClawxError, Result};
use clawx_types::traits::NotificationPort;
use sqlx::SqlitePool;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Row type
// ---------------------------------------------------------------------------

#[derive(Debug, sqlx::FromRow)]
struct NotificationRow {
    id: String,
    run_id: String,
    channel_kind: String,
    target_ref: Option<String>,
    delivery_status: String,
    suppression_reason: Option<String>,
    payload_summary: Option<String>,
    delivered_at: Option<String>,
    created_at: String,
}

impl TryFrom<NotificationRow> for TaskNotification {
    type Error = ClawxError;

    fn try_from(row: NotificationRow) -> Result<Self> {
        Ok(TaskNotification {
            id: TaskNotificationId(
                uuid::Uuid::parse_str(&row.id)
                    .map_err(|e| ClawxError::Database(format!("invalid notification id: {}", e)))?,
            ),
            run_id: RunId::from_str(&row.run_id)
                .map_err(|e| ClawxError::Database(format!("invalid run id: {}", e)))?,
            channel_kind: row.channel_kind,
            target_ref: row.target_ref,
            delivery_status: DeliveryStatus::from_str(&row.delivery_status)
                .map_err(|e| ClawxError::Database(format!("invalid delivery_status: {}", e)))?,
            suppression_reason: row.suppression_reason,
            payload_summary: row.payload_summary,
            delivered_at: row
                .delivered_at
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| ClawxError::Database(format!("invalid delivered_at: {}", e)))
                })
                .transpose()?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
        })
    }
}

// ---------------------------------------------------------------------------
// SqliteNotificationRepo
// ---------------------------------------------------------------------------

/// Notification repository backed by SQLite.
pub struct SqliteNotificationRepo {
    pool: SqlitePool,
}

impl SqliteNotificationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl NotificationPort for SqliteNotificationRepo {
    async fn send(&self, notification: TaskNotification) -> Result<()> {
        let id = notification.id.to_string();
        let run_id = notification.run_id.to_string();
        let delivery_status = notification.delivery_status.to_string();
        let delivered_at = notification.delivered_at.map(|dt| dt.to_rfc3339());
        let created_at = notification.created_at.to_rfc3339();

        sqlx::query(
            "INSERT INTO task_notifications (id, run_id, channel_kind, target_ref, delivery_status,
             suppression_reason, payload_summary, delivered_at, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&run_id)
        .bind(&notification.channel_kind)
        .bind(&notification.target_ref)
        .bind(&delivery_status)
        .bind(&notification.suppression_reason)
        .bind(&notification.payload_summary)
        .bind(&delivered_at)
        .bind(&created_at)
        .execute(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("insert notification: {}", e)))?;

        Ok(())
    }

    async fn query_status(&self, run_id: RunId) -> Result<Vec<TaskNotification>> {
        let rows: Vec<NotificationRow> = sqlx::query_as(
            "SELECT * FROM task_notifications WHERE run_id = ? ORDER BY created_at DESC",
        )
        .bind(run_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ClawxError::Database(format!("query notifications: {}", e)))?;

        rows.into_iter().map(TaskNotification::try_from).collect()
    }
}

/// Create a notification record for a suppressed delivery.
pub fn suppressed_notification(
    run_id: RunId,
    channel_kind: &str,
    reason: &str,
) -> TaskNotification {
    let now = Utc::now();
    TaskNotification {
        id: TaskNotificationId::new(),
        run_id,
        channel_kind: channel_kind.to_string(),
        target_ref: None,
        delivery_status: DeliveryStatus::Suppressed,
        suppression_reason: Some(reason.to_string()),
        payload_summary: None,
        delivered_at: None,
        created_at: now,
    }
}

/// Create a notification record for a successful delivery.
pub fn sent_notification(
    run_id: RunId,
    channel_kind: &str,
    target_ref: Option<&str>,
    summary: Option<&str>,
) -> TaskNotification {
    let now = Utc::now();
    TaskNotification {
        id: TaskNotificationId::new(),
        run_id,
        channel_kind: channel_kind.to_string(),
        target_ref: target_ref.map(String::from),
        delivery_status: DeliveryStatus::Sent,
        suppression_reason: None,
        payload_summary: summary.map(String::from),
        delivered_at: Some(now),
        created_at: now,
    }
}

/// Create a notification record for a failed delivery.
pub fn failed_notification(
    run_id: RunId,
    channel_kind: &str,
    reason: &str,
) -> TaskNotification {
    let now = Utc::now();
    TaskNotification {
        id: TaskNotificationId::new(),
        run_id,
        channel_kind: channel_kind.to_string(),
        target_ref: None,
        delivery_status: DeliveryStatus::Failed,
        suppression_reason: Some(reason.to_string()),
        payload_summary: None,
        delivered_at: None,
        created_at: now,
    }
}

/// Desktop notification adapter using macOS Notification Center.
/// Falls back to logging if osascript is not available.
pub struct DesktopNotifier;

impl DesktopNotifier {
    /// Send a macOS desktop notification.
    pub async fn notify(title: &str, message: &str) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            let script = format!(
                "display notification \"{}\" with title \"{}\"",
                message.replace('"', "\\\""),
                title.replace('"', "\\\"")
            );
            let status = tokio::process::Command::new("osascript")
                .args(["-e", &script])
                .status()
                .await
                .map_err(|e| ClawxError::Internal(format!("osascript failed: {}", e)))?;
            if !status.success() {
                tracing::warn!("desktop notification failed (exit {})", status);
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            tracing::info!(title, message, "desktop notification (not on macOS)");
        }
        Ok(())
    }
}

/// File-based notification adapter — writes result to a file.
pub struct FileNotifier;

impl FileNotifier {
    /// Write notification content to a file.
    pub async fn write_to_file(path: &str, content: &str) -> Result<()> {
        tokio::fs::write(path, content)
            .await
            .map_err(|e| ClawxError::Internal(format!("write notification file: {}", e)))?;
        Ok(())
    }
}

/// Compute negative feedback rate for a task.
/// Returns (rejected + mute_forever) / total, or 0.0 if no feedback.
pub async fn negative_feedback_rate(pool: &SqlitePool, task_id: &str) -> Result<f64> {
    let total: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM task_runs WHERE task_id = ? AND feedback_kind IS NOT NULL",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("count feedback: {}", e)))?;

    if total.0 == 0 {
        return Ok(0.0);
    }

    let negative: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM task_runs WHERE task_id = ? AND feedback_kind IN ('rejected', 'mute_forever')",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("count negative feedback: {}", e)))?;

    Ok(negative.0 as f64 / total.0 as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    async fn setup() -> Database {
        let db = Database::in_memory().await.unwrap();
        // Insert a dummy agent and task for FK constraints
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO agents (id, name, role, model_id, status, capabilities, created_at, updated_at)
             VALUES (?, 'test', 'assistant', 'default', 'idle', '[]', ?, ?)",
        )
        .bind("00000000-0000-0000-0000-000000000001")
        .bind(&now)
        .bind(&now)
        .execute(&db.main)
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, agent_id, name, goal, source_kind, lifecycle_status, notification_policy, suppression_state, created_at, updated_at)
             VALUES (?, ?, 'test-task', 'test goal', 'manual', 'active', '{}', 'normal', ?, ?)",
        )
        .bind("00000000-0000-0000-0000-000000000010")
        .bind("00000000-0000-0000-0000-000000000001")
        .bind(&now)
        .bind(&now)
        .execute(&db.main)
        .await
        .unwrap();

        db
    }

    async fn insert_run(pool: &SqlitePool, task_id: &str, key: &str) -> RunId {
        let run_id = RunId::new();
        let now = Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO task_runs (id, task_id, idempotency_key, run_status, checkpoint, notification_status, triggered_at, created_at)
             VALUES (?, ?, ?, 'queued', '{}', 'pending', ?, ?)",
        )
        .bind(run_id.to_string())
        .bind(task_id)
        .bind(key)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .unwrap();
        run_id
    }

    // -----------------------------------------------------------------------
    // Notification CRUD
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn send_and_query_notification() {
        let db = setup().await;
        let task_id = "00000000-0000-0000-0000-000000000010";
        let run_id = insert_run(&db.main, task_id, "key-notif-1").await;

        let repo = SqliteNotificationRepo::new(db.main.clone());
        let notification = sent_notification(run_id, "desktop", None, Some("Task completed"));

        repo.send(notification).await.unwrap();

        let notifications = repo.query_status(run_id).await.unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].channel_kind, "desktop");
        assert_eq!(notifications[0].delivery_status, DeliveryStatus::Sent);
        assert_eq!(
            notifications[0].payload_summary.as_deref(),
            Some("Task completed")
        );
    }

    #[tokio::test]
    async fn send_suppressed_notification() {
        let db = setup().await;
        let task_id = "00000000-0000-0000-0000-000000000010";
        let run_id = insert_run(&db.main, task_id, "key-notif-2").await;

        let repo = SqliteNotificationRepo::new(db.main.clone());
        let notification = suppressed_notification(run_id, "desktop", "cooldown window");

        repo.send(notification).await.unwrap();

        let notifications = repo.query_status(run_id).await.unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].delivery_status, DeliveryStatus::Suppressed);
        assert_eq!(
            notifications[0].suppression_reason.as_deref(),
            Some("cooldown window")
        );
    }

    #[tokio::test]
    async fn query_empty_returns_empty() {
        let db = setup().await;
        let repo = SqliteNotificationRepo::new(db.main.clone());
        let run_id = RunId::new();
        let notifications = repo.query_status(run_id).await.unwrap();
        assert!(notifications.is_empty());
    }

    #[tokio::test]
    async fn multiple_notifications_per_run() {
        let db = setup().await;
        let task_id = "00000000-0000-0000-0000-000000000010";
        let run_id = insert_run(&db.main, task_id, "key-notif-3").await;

        let repo = SqliteNotificationRepo::new(db.main.clone());
        repo.send(sent_notification(run_id, "desktop", None, Some("n1")))
            .await
            .unwrap();
        repo.send(sent_notification(run_id, "file", Some("/tmp/n.txt"), Some("n2")))
            .await
            .unwrap();
        repo.send(failed_notification(run_id, "telegram", "bot token expired"))
            .await
            .unwrap();

        let notifications = repo.query_status(run_id).await.unwrap();
        assert_eq!(notifications.len(), 3);
    }

    // -----------------------------------------------------------------------
    // Negative feedback rate
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn negative_feedback_rate_no_feedback() {
        let db = setup().await;
        let task_id = "00000000-0000-0000-0000-000000000010";
        let rate = negative_feedback_rate(&db.main, task_id).await.unwrap();
        assert_eq!(rate, 0.0);
    }

    #[tokio::test]
    async fn negative_feedback_rate_mixed() {
        let db = setup().await;
        let task_id = "00000000-0000-0000-0000-000000000010";

        // Create runs with feedback
        let r1 = insert_run(&db.main, task_id, "fb-1").await;
        let r2 = insert_run(&db.main, task_id, "fb-2").await;
        let r3 = insert_run(&db.main, task_id, "fb-3").await;
        let r4 = insert_run(&db.main, task_id, "fb-4").await;

        sqlx::query("UPDATE task_runs SET feedback_kind = 'accepted' WHERE id = ?")
            .bind(r1.to_string())
            .execute(&db.main)
            .await
            .unwrap();
        sqlx::query("UPDATE task_runs SET feedback_kind = 'rejected' WHERE id = ?")
            .bind(r2.to_string())
            .execute(&db.main)
            .await
            .unwrap();
        sqlx::query("UPDATE task_runs SET feedback_kind = 'ignored' WHERE id = ?")
            .bind(r3.to_string())
            .execute(&db.main)
            .await
            .unwrap();
        sqlx::query("UPDATE task_runs SET feedback_kind = 'mute_forever' WHERE id = ?")
            .bind(r4.to_string())
            .execute(&db.main)
            .await
            .unwrap();

        let rate = negative_feedback_rate(&db.main, task_id).await.unwrap();
        // 2 negative (rejected + mute_forever) out of 4 total = 0.5
        assert!((rate - 0.5).abs() < 0.001);
    }

    // -----------------------------------------------------------------------
    // Helper constructors
    // -----------------------------------------------------------------------

    #[test]
    fn sent_notification_has_correct_status() {
        let n = sent_notification(RunId::new(), "desktop", None, Some("done"));
        assert_eq!(n.delivery_status, DeliveryStatus::Sent);
        assert!(n.delivered_at.is_some());
    }

    #[test]
    fn suppressed_notification_has_reason() {
        let n = suppressed_notification(RunId::new(), "desktop", "quiet hours");
        assert_eq!(n.delivery_status, DeliveryStatus::Suppressed);
        assert_eq!(n.suppression_reason.as_deref(), Some("quiet hours"));
    }

    #[test]
    fn failed_notification_has_reason() {
        let n = failed_notification(RunId::new(), "telegram", "timeout");
        assert_eq!(n.delivery_status, DeliveryStatus::Failed);
        assert_eq!(n.suppression_reason.as_deref(), Some("timeout"));
    }
}
