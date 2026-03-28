//! Channel CRUD repository -- thin layer over SQLite `channels` table.

use chrono::Utc;
use clawx_types::channel::{Channel, ChannelStatus, ChannelType};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::{AgentId, ChannelId};
use clawx_types::traits::ChannelUpdate;
use sqlx::SqlitePool;
use std::str::FromStr;

/// Row returned by SELECT on the channels table.
#[derive(Debug, sqlx::FromRow)]
struct ChannelRow {
    id: String,
    #[sqlx(rename = "type")]
    channel_type: String,
    name: String,
    config: String,
    agent_id: Option<String>,
    status: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ChannelRow> for Channel {
    type Error = ClawxError;

    fn try_from(row: ChannelRow) -> Result<Self> {
        Ok(Channel {
            id: ChannelId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid channel id: {}", e)))?,
            channel_type: ChannelType::from_str(&row.channel_type)
                .map_err(|e| ClawxError::Database(format!("invalid channel type: {}", e)))?,
            name: row.name,
            config: serde_json::from_str(&row.config)
                .map_err(|e| ClawxError::Database(format!("invalid config json: {}", e)))?,
            agent_id: row
                .agent_id
                .map(|s| {
                    AgentId::from_str(&s)
                        .map_err(|e| ClawxError::Database(format!("invalid agent_id: {}", e)))
                })
                .transpose()?,
            status: ChannelStatus::from_str(&row.status)
                .map_err(|e| ClawxError::Database(format!("invalid channel status: {}", e)))?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid updated_at: {}", e)))?,
        })
    }
}

/// Insert a new channel.
pub async fn create_channel(pool: &SqlitePool, channel: &Channel) -> Result<Channel> {
    let id = channel.id.to_string();
    let channel_type = channel.channel_type.to_string();
    let config = serde_json::to_string(&channel.config)
        .map_err(|e| ClawxError::Internal(format!("serialize config: {}", e)))?;
    let status = channel.status.to_string();
    let created_at = channel.created_at.to_rfc3339();
    let updated_at = channel.updated_at.to_rfc3339();
    let agent_id = channel.agent_id.map(|a| a.to_string());

    sqlx::query(
        "INSERT INTO channels (id, type, name, config, agent_id, status, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&channel_type)
    .bind(&channel.name)
    .bind(&config)
    .bind(&agent_id)
    .bind(&status)
    .bind(&created_at)
    .bind(&updated_at)
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("create channel: {}", e)))?;

    get_channel(pool, &channel.id)
        .await?
        .ok_or_else(|| ClawxError::Internal("channel not found after insert".into()))
}

/// Get a single channel by ID.
pub async fn get_channel(pool: &SqlitePool, id: &ChannelId) -> Result<Option<Channel>> {
    let row: Option<ChannelRow> =
        sqlx::query_as("SELECT * FROM channels WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("get channel: {}", e)))?;

    row.map(Channel::try_from).transpose()
}

/// List all channels.
pub async fn list_channels(pool: &SqlitePool) -> Result<Vec<Channel>> {
    let rows: Vec<ChannelRow> =
        sqlx::query_as("SELECT * FROM channels ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("list channels: {}", e)))?;

    rows.into_iter().map(Channel::try_from).collect()
}

/// Update a channel (partial merge).
pub async fn update_channel(
    pool: &SqlitePool,
    id: &ChannelId,
    updates: &ChannelUpdate,
) -> Result<Channel> {
    let existing = get_channel(pool, id)
        .await?
        .ok_or_else(|| ClawxError::NotFound {
            entity: "channel".into(),
            id: id.to_string(),
        })?;

    let name = updates.name.as_deref().unwrap_or(&existing.name);
    let config = updates.config.as_ref().unwrap_or(&existing.config);
    let config_json = serde_json::to_string(config)
        .map_err(|e| ClawxError::Internal(format!("serialize config: {}", e)))?;
    let agent_id = match &updates.agent_id {
        Some(aid) => Some(aid.to_string()),
        None => existing.agent_id.map(|a| a.to_string()),
    };
    let status = updates.status.unwrap_or(existing.status);
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE channels SET name = ?, config = ?, agent_id = ?, status = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(name)
    .bind(&config_json)
    .bind(&agent_id)
    .bind(status.to_string())
    .bind(&now)
    .bind(id.to_string())
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("update channel: {}", e)))?;

    get_channel(pool, id)
        .await?
        .ok_or_else(|| ClawxError::Internal("channel not found after update".into()))
}

/// Delete a channel by ID.
pub async fn delete_channel(pool: &SqlitePool, id: &ChannelId) -> Result<()> {
    let result = sqlx::query("DELETE FROM channels WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("delete channel: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ClawxError::NotFound {
            entity: "channel".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn make_channel(name: &str) -> Channel {
        let now = Utc::now();
        Channel {
            id: ChannelId::new(),
            channel_type: ChannelType::Telegram,
            name: name.to_string(),
            config: serde_json::json!({"token": "abc123"}),
            agent_id: None,
            status: ChannelStatus::Disconnected,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn create_and_get_channel() {
        let db = Database::in_memory().await.unwrap();
        let ch = make_channel("Test Channel");
        let created = create_channel(&db.main, &ch).await.unwrap();
        assert_eq!(created.name, "Test Channel");
        assert_eq!(created.channel_type, ChannelType::Telegram);
        assert_eq!(created.status, ChannelStatus::Disconnected);

        let fetched = get_channel(&db.main, &ch.id).await.unwrap().unwrap();
        assert_eq!(fetched.id, ch.id);
        assert_eq!(fetched.name, "Test Channel");
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let db = Database::in_memory().await.unwrap();
        let result = get_channel(&db.main, &ChannelId::new()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn list_channels_returns_all() {
        let db = Database::in_memory().await.unwrap();
        create_channel(&db.main, &make_channel("A")).await.unwrap();
        create_channel(&db.main, &make_channel("B")).await.unwrap();
        create_channel(&db.main, &make_channel("C")).await.unwrap();

        let channels = list_channels(&db.main).await.unwrap();
        assert_eq!(channels.len(), 3);
    }

    #[tokio::test]
    async fn update_channel_partial_name_and_status() {
        let db = Database::in_memory().await.unwrap();
        let ch = make_channel("Original");
        create_channel(&db.main, &ch).await.unwrap();

        let updates = ChannelUpdate {
            name: Some("Renamed".to_string()),
            status: Some(ChannelStatus::Connected),
            ..Default::default()
        };
        let updated = update_channel(&db.main, &ch.id, &updates).await.unwrap();
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.status, ChannelStatus::Connected);
        // config should be unchanged
        assert_eq!(updated.config, serde_json::json!({"token": "abc123"}));
    }

    #[tokio::test]
    async fn update_nonexistent_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = update_channel(
            &db.main,
            &ChannelId::new(),
            &ChannelUpdate::default(),
        )
        .await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn delete_channel_removes_it() {
        let db = Database::in_memory().await.unwrap();
        let ch = make_channel("ToDelete");
        create_channel(&db.main, &ch).await.unwrap();

        delete_channel(&db.main, &ch.id).await.unwrap();
        let fetched = get_channel(&db.main, &ch.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = delete_channel(&db.main, &ChannelId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn config_roundtrips_as_json() {
        let db = Database::in_memory().await.unwrap();
        let mut ch = make_channel("JsonTest");
        ch.config = serde_json::json!({
            "webhook_url": "https://example.com/hook",
            "nested": {"key": [1, 2, 3]}
        });
        create_channel(&db.main, &ch).await.unwrap();

        let fetched = get_channel(&db.main, &ch.id).await.unwrap().unwrap();
        assert_eq!(fetched.config["webhook_url"], "https://example.com/hook");
        assert_eq!(fetched.config["nested"]["key"][1], 2);
    }

    #[tokio::test]
    async fn update_channel_config() {
        let db = Database::in_memory().await.unwrap();
        let ch = make_channel("ConfigUpdate");
        create_channel(&db.main, &ch).await.unwrap();

        let updates = ChannelUpdate {
            config: Some(serde_json::json!({"new_token": "xyz"})),
            ..Default::default()
        };
        let updated = update_channel(&db.main, &ch.id, &updates).await.unwrap();
        assert_eq!(updated.config["new_token"], "xyz");
        // old config key should be gone
        assert!(updated.config.get("token").is_none());
    }
}
