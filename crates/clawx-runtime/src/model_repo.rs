//! LLM provider CRUD repository — thin layer over SQLite `llm_providers` table.

use chrono::Utc;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::ProviderId;
use clawx_types::llm::{LlmProviderConfig, ProviderType};
use sqlx::SqlitePool;
use std::str::FromStr;

/// Row returned by SELECT on the llm_providers table.
#[derive(Debug, sqlx::FromRow)]
struct ProviderRow {
    id: String,
    name: String,
    #[sqlx(rename = "type")]
    provider_type: String,
    base_url: String,
    model_name: String,
    parameters: String,
    is_default: i32,
    created_at: String,
    updated_at: String,
}

impl TryFrom<ProviderRow> for LlmProviderConfig {
    type Error = ClawxError;

    fn try_from(row: ProviderRow) -> Result<Self> {
        Ok(LlmProviderConfig {
            id: ProviderId::from_str(&row.id)
                .map_err(|e| ClawxError::Database(format!("invalid provider id: {}", e)))?,
            name: row.name,
            provider_type: ProviderType::from_str(&row.provider_type)
                .map_err(|e| ClawxError::Database(format!("invalid provider type: {}", e)))?,
            base_url: row.base_url,
            model_name: row.model_name,
            parameters: serde_json::from_str(&row.parameters)
                .map_err(|e| ClawxError::Database(format!("invalid parameters json: {}", e)))?,
            is_default: row.is_default != 0,
            created_at: chrono::DateTime::parse_from_rfc3339(&row.created_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid created_at: {}", e)))?,
            updated_at: chrono::DateTime::parse_from_rfc3339(&row.updated_at)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|e| ClawxError::Database(format!("invalid updated_at: {}", e)))?,
        })
    }
}

/// Insert a new LLM provider.
pub async fn create_provider(
    pool: &SqlitePool,
    config: &LlmProviderConfig,
) -> Result<LlmProviderConfig> {
    let id = config.id.to_string();
    let provider_type = config.provider_type.to_string();
    let parameters = serde_json::to_string(&config.parameters)
        .map_err(|e| ClawxError::Internal(format!("serialize parameters: {}", e)))?;
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "INSERT INTO llm_providers (id, name, type, base_url, model_name, parameters, is_default, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&config.name)
    .bind(&provider_type)
    .bind(&config.base_url)
    .bind(&config.model_name)
    .bind(&parameters)
    .bind(config.is_default as i32)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("create provider: {}", e)))?;

    get_provider(pool, &config.id)
        .await?
        .ok_or_else(|| ClawxError::Internal("provider not found after insert".into()))
}

/// List all LLM providers.
pub async fn list_providers(pool: &SqlitePool) -> Result<Vec<LlmProviderConfig>> {
    let rows: Vec<ProviderRow> =
        sqlx::query_as("SELECT * FROM llm_providers ORDER BY created_at DESC")
            .fetch_all(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("list providers: {}", e)))?;

    rows.into_iter().map(LlmProviderConfig::try_from).collect()
}

/// Get a single provider by ID.
pub async fn get_provider(
    pool: &SqlitePool,
    id: &ProviderId,
) -> Result<Option<LlmProviderConfig>> {
    let row: Option<ProviderRow> =
        sqlx::query_as("SELECT * FROM llm_providers WHERE id = ?")
            .bind(id.to_string())
            .fetch_optional(pool)
            .await
            .map_err(|e| ClawxError::Database(format!("get provider: {}", e)))?;

    row.map(LlmProviderConfig::try_from).transpose()
}

/// Partial update payload for an LLM provider.
#[derive(Debug, Default, serde::Deserialize)]
pub struct ProviderUpdate {
    pub name: Option<String>,
    pub provider_type: Option<String>,
    pub base_url: Option<String>,
    pub model_name: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub is_default: Option<bool>,
}

/// Update an LLM provider (partial merge).
pub async fn update_provider(
    pool: &SqlitePool,
    id: &ProviderId,
    updates: &ProviderUpdate,
) -> Result<LlmProviderConfig> {
    let existing = get_provider(pool, id)
        .await?
        .ok_or_else(|| ClawxError::NotFound {
            entity: "provider".into(),
            id: id.to_string(),
        })?;

    let name = updates.name.as_deref().unwrap_or(&existing.name);
    let existing_provider_type = existing.provider_type.to_string();
    let provider_type = updates
        .provider_type
        .as_deref()
        .unwrap_or(&existing_provider_type);
    let base_url = updates.base_url.as_deref().unwrap_or(&existing.base_url);
    let model_name = updates
        .model_name
        .as_deref()
        .unwrap_or(&existing.model_name);
    let parameters = match &updates.parameters {
        Some(v) => serde_json::to_string(v),
        None => serde_json::to_string(&existing.parameters),
    }
    .map_err(|e| ClawxError::Internal(format!("serialize parameters: {}", e)))?;
    let is_default = updates.is_default.unwrap_or(existing.is_default);
    let now = Utc::now().to_rfc3339();

    sqlx::query(
        "UPDATE llm_providers SET name = ?, type = ?, base_url = ?, model_name = ?, parameters = ?, is_default = ?, updated_at = ?
         WHERE id = ?",
    )
    .bind(name)
    .bind(provider_type)
    .bind(base_url)
    .bind(model_name)
    .bind(&parameters)
    .bind(is_default as i32)
    .bind(&now)
    .bind(id.to_string())
    .execute(pool)
    .await
    .map_err(|e| ClawxError::Database(format!("update provider: {}", e)))?;

    get_provider(pool, id)
        .await?
        .ok_or_else(|| ClawxError::Internal("provider not found after update".into()))
}

/// Delete a provider by ID.
pub async fn delete_provider(pool: &SqlitePool, id: &ProviderId) -> Result<()> {
    let result = sqlx::query("DELETE FROM llm_providers WHERE id = ?")
        .bind(id.to_string())
        .execute(pool)
        .await
        .map_err(|e| ClawxError::Database(format!("delete provider: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(ClawxError::NotFound {
            entity: "provider".into(),
            id: id.to_string(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    fn make_provider(name: &str) -> LlmProviderConfig {
        let now = Utc::now();
        LlmProviderConfig {
            id: ProviderId::new(),
            name: name.to_string(),
            provider_type: ProviderType::Anthropic,
            base_url: "https://api.anthropic.com".to_string(),
            model_name: "claude-3-opus".to_string(),
            parameters: serde_json::json!({"temperature": 0.7}),
            is_default: false,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn create_and_get_provider() {
        let db = Database::in_memory().await.unwrap();
        let provider = make_provider("Test Provider");
        let created = create_provider(&db.main, &provider).await.unwrap();
        assert_eq!(created.name, "Test Provider");
        assert_eq!(created.provider_type, ProviderType::Anthropic);

        let fetched = get_provider(&db.main, &provider.id).await.unwrap().unwrap();
        assert_eq!(fetched.name, "Test Provider");
        assert_eq!(fetched.model_name, "claude-3-opus");
    }

    #[tokio::test]
    async fn list_providers_returns_all() {
        let db = Database::in_memory().await.unwrap();
        create_provider(&db.main, &make_provider("A")).await.unwrap();
        create_provider(&db.main, &make_provider("B")).await.unwrap();

        let providers = list_providers(&db.main).await.unwrap();
        assert_eq!(providers.len(), 2);
    }

    #[tokio::test]
    async fn update_provider_partial() {
        let db = Database::in_memory().await.unwrap();
        let provider = make_provider("Original");
        create_provider(&db.main, &provider).await.unwrap();

        let updates = ProviderUpdate {
            name: Some("Renamed".to_string()),
            ..Default::default()
        };
        let updated = update_provider(&db.main, &provider.id, &updates).await.unwrap();
        assert_eq!(updated.name, "Renamed");
        assert_eq!(updated.provider_type, ProviderType::Anthropic); // unchanged
        assert_eq!(updated.model_name, "claude-3-opus"); // unchanged
    }

    #[tokio::test]
    async fn delete_provider_removes_it() {
        let db = Database::in_memory().await.unwrap();
        let provider = make_provider("ToDelete");
        create_provider(&db.main, &provider).await.unwrap();

        delete_provider(&db.main, &provider.id).await.unwrap();
        let fetched = get_provider(&db.main, &provider.id).await.unwrap();
        assert!(fetched.is_none());
    }

    #[tokio::test]
    async fn delete_nonexistent_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = delete_provider(&db.main, &ProviderId::new()).await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let db = Database::in_memory().await.unwrap();
        let result = get_provider(&db.main, &ProviderId::new()).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn update_nonexistent_returns_not_found() {
        let db = Database::in_memory().await.unwrap();
        let result = update_provider(
            &db.main,
            &ProviderId::new(),
            &ProviderUpdate::default(),
        )
        .await;
        assert!(matches!(result, Err(ClawxError::NotFound { .. })));
    }

    #[tokio::test]
    async fn provider_is_default_flag() {
        let db = Database::in_memory().await.unwrap();
        let mut provider = make_provider("Default");
        provider.is_default = true;
        let created = create_provider(&db.main, &provider).await.unwrap();
        assert!(created.is_default);

        let updates = ProviderUpdate {
            is_default: Some(false),
            ..Default::default()
        };
        let updated = update_provider(&db.main, &provider.id, &updates).await.unwrap();
        assert!(!updated.is_default);
    }
}
