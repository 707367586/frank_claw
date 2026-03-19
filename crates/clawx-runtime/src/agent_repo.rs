//! Agent CRUD repository — thin layer over SQLite `agents` table.

use chrono::Utc;
use clawx_types::agent::{AgentConfig, AgentStatus};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::{AgentId, ProviderId};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Row returned by SELECT on the agents table.
#[derive(Debug, sqlx::FromRow)]
struct AgentRow {
    id: String,
    name: String,
    role: String,
    system_prompt: Option<String>,
    model_id: String,
    icon: Option<String>,
    status: String,
    capabilities: String,
    created_at: String,
    updated_at: String,
    last_active_at: Option<String>,
}

impl TryFrom<AgentRow> for AgentConfig {
    type Error = ClawxError;

    fn try_from(row: AgentRow) -> Result<Self> {
        Ok(AgentConfig {
            id: AgentId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid agent id: {}", e)))?,
            name: row.name,
            role: row.role,
            system_prompt: row.system_prompt,
            model_id: ProviderId::from_str(&row.model_id)
                .map_err(|e| ClawxError::Database(format!("invalid provider id: {}", e)))?,
            icon: row.icon,
            status: AgentStatus::from_str(&row.status)
                .map_err(|e| ClawxError::Database(format!("invalid status: {}", e)))?,
            capabilities: serde_json::from_str(&row.capabilities)
                .map_err(|e| ClawxError::Database(format!("invalid capabilities json: {}", e)))?,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid updated_at: {}", e)))?,
            last_active_at: row
                .last_active_at
                .map(|s| {
                    chrono::DateTime::parse_from_rfc3339(&s)
                        .map(|dt| dt.with_timezone(&Utc))
                        .map_err(|e| ClawxError::Database(format!("invalid last_active_at: {}", e)))
                })
                .transpose()?,
        })
    }
}

/// Insert a new agent.
pub async fn create_agent(pool: &SqlitePool, agent: &AgentConfig) -> Result<AgentConfig> {
    let id = agent.id.to_string();
    let model_id = agent.model_id.to_string();
    let status = agent.status.to_string();
    let capabilities = serde_json::to_string(&agent.capabilities)
        .map_err(|e| ClawxError::Internal(format!("serialize capabilities: {}", e)))?;
    let created_at = agent.created_at.to_rfc3339();
    let updated_at = agent.updated_at.to_rfc3339();
    let last_active_at = agent.last_active_at.map(|dt| dt.to_rfc3339());

    sqlx::query(
        "INSERT INTO agents (id, name, role, system_prompt, model_id, icon, status, capabilities, created_at, updated_at, last_active_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&agent.name)
    .bind(&agent.role)
    .bind(&agent.system_prompt)
    .bind(&model_id)
    .bind(&agent.icon)
    .bind(&status)
    .bind(&capabilities)
    .bind(&created_at)
    .bind(&updated_at)
    .bind(&last_active_at)
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("create agent: {}", e)))?;

    get_agent(pool, &agent.id)
        .await?
        .ok_or_else(|| ClawxError::Internal("agent not found after insert".into()))
}

/// Get a single agent by ID.
pub async fn get_agent(pool: &SqlitePool, id: &AgentId) -> Result<Option<AgentConfig>> {
    let row: Option<AgentRow> =
        sqlx::query_as("SELECT * FROM agents WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("get agent: {}", e)))?;

    row.map(AgentConfig::try_from).transpose()
}

/// List all agents.
pub async fn list_agents(pool: &SqlitePool) -> Result<Vec<AgentConfig>> {
    let rows: Vec<AgentRow> =
        sqlx::query_as("SELECT * FROM agents ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("list agents: {}", e)))?;

    rows.into_iter().map(AgentConfig::try_from).collect()
}

/// Partial update payload.
#[derive(Debug, Default, serde::Deserialize)]
pub struct AgentUpdate {
    pub name: Option<String>,
    pub role: Option<String>,
    pub system_prompt: Option<Option<String>>,
    pub model_id: Option<String>,
    pub icon: Option<Option<String>>,
    pub capabilities: Option<Vec<String>>,
}

/// Update an agent (partial merge).
pub async fn update_agent(
    pool: &SqlitePool,
    id: &AgentId,
    updates: &AgentUpdate,
) -> Result<AgentConfig> {
    let existing = get_agent(pool, id)
        .await?
        .ok_or_else(|| ClawxError::NotFound {
            entity: "agent".into(),
            id: id.to_string(),
        })?;

    let name = updates.name.as_deref().unwrap_or(&existing.name);
    let role = updates.role.as_deref().unwrap_or(&existing.role);
    let system_prompt = match &updates.system_prompt {
        Some(v) => v.as_deref(),
        None => existing.system_prompt.as_deref(),
    };
    let existing_model_id = existing.model_id.to_string();
    let model_id_str = updates
        .model_id
        .as_deref()
        .unwrap_or(&existing_model_id);
    let icon = match &updates.icon {
        Some(v) => v.as_deref(),
        None => existing.icon.as_deref(),
    };
    let capabilities = match &updates.capabilities {
        Some(v) => v,
        None => &existing.capabilities,
    };
    let caps_json = serde_json::to_string(capabilities)
        .map_err(|e| ClawxError::Internal(format!("serialize capabilities: {}", e)))?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE agents SET name = ?, role = ?, system_prompt = ?, model_id = ?, icon = ?, capabilities = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(name)
    .bind(role)
    .bind(system_prompt)
    .bind(model_id_str)
    .bind(icon)
    .bind(&caps_json)
    .bind(&now)
    .bind(id.to_string())
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("update agent: {}", e)))?;

    get_agent(pool, id)
        .await?
        .ok_or_else(|| ClawxError::Internal("agent not found after update".into()))
}

/// Delete an agent by ID.
pub async fn delete_agent(pool: &SqlitePool, id: &AgentId) -> Result<()> {
    let result = sqlx::query("DELETE FROM agents WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("delete agent: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ClawxError::NotFound {
            entity: "agent".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

/// Clone an existing agent with a new ID and "(Copy)" suffix on name.
pub async fn clone_agent(pool: &SqlitePool, id: &AgentId) -> Result<AgentConfig> {
    let original = get_agent(pool, id)
        .await?
        .ok_or_else(|| ClawxError::NotFound {
            entity: "agent".into(),
            id: id.to_string(),
        })?;

    let now = Utc::now();
    let cloned = AgentConfig {
        id: AgentId::new(),
        name: format!("{} (Copy)", original.name),
        status: AgentStatus::Idle,
        created_at: now,
        updated_at: now,
        last_active_at: None,
        ..original
    };

    create_agent(pool, &cloned).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn make_agent(name: &str) -> AgentConfig {
        let now = Utc::now();
        AgentConfig {
            id: AgentId::new(),
            name: name.to_string(),
            role: "assistant".to_string(),
            system_prompt: Some("You are helpful.".to_string()),
            model_id: ProviderId::new(),
            icon: None,
            status: AgentStatus::Idle,
            capabilities: vec!["web_search".to_string()],
            created_at: now,
            updated_at: now,
            last_active_at: None,
        }
    }

    #[tokio::test]
    async fn create_and_get_agent() {
        let db = Database::in_memory().await.unwrap();
        let agent = make_agent("Test Agent");
        let created = create_agent(&db.main, &agent).await.unwrap();
        assert_eq!(created.name, "Test Agent");
        assert_eq!(created.id, agent.id);

        let fetched = get_agent(&db.main, &agent.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Test Agent");
        assert_eq!(fetched.capabilities, vec!["web_search".to_string()]);
    }

    #[tokio::test]
    async fn list_agents_returns_all() {
        let db = Database::in_memory().await.unwrap();
        create_agent(&db.main, &make_agent("A")).await.unwrap();
        create_agent(&db.main, &make_agent("B")).await.unwrap();

        let agents = list_agents(&db.main).await.unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[tokio::test]
    async fn update_agent_partial() {
        let db = Database::in_memory().await.unwrap();
        let agent = make_agent("Original");
        create_agent(&db.main, &agent).await.unwrap();

        let updates = AgentUpdate {
            name: Some("Renamed".to_string()),
            ..Default::default()
        };
        let updated = update_agent(&db.main, &agent.id, &updates).await.unwrap();
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.role, "assistant"); // unchanged
    }

    #[tokio::test]
    async fn delete_agent_removes_it() {
        let db = Database::in_memory().await.unwrap();
        let agent = make_agent("ToDelete");
        create_agent(&db.main, &agent).await.unwrap();

        delete_agent(&db.main, &agent.id).await.unwrap();
        let fetched = get_agent(&db.main, &agent.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = delete_agent(&db.main, &AgentId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn clone_agent_creates_copy() {
        let db = Database::in_memory().await.unwrap();
        let agent = make_agent("Original");
        create_agent(&db.main, &agent).await.unwrap();

        let cloned = clone_agent(&db.main, &agent.id).await.unwrap();
        assert_eq!(cloned.name, "Original (Copy)");
        assert_ne!(cloned.id, agent.id);
        assert_eq!(cloned.role, agent.role);
        assert_eq!(cloned.capabilities, agent.capabilities);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let db = Database::in_memory().await.unwrap();
        let result = get_agent(&db.main, &AgentId::new()).await.unwrap();
        assert!(result.is_none());
    }
}
