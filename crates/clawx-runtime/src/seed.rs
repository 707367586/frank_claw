//! Seed default LLM provider and Agents on first launch.
//!
//! Ensures a freshly-initialized database has a usable state: one default
//! provider plus a handful of starter Agents, so the GUI never shows an
//! empty-list landing page.

use chrono::Utc;
use clawx_types::agent::{AgentConfig, AgentStatus};
use clawx_types::error::Result;
use clawx_types::ids::{AgentId, ProviderId};
use clawx_types::llm::{LlmProviderConfig, ProviderType};
use serde_json::json;
use sqlx::SqlitePool;

use crate::{agent_repo, model_repo};

struct DefaultAgent {
    name: &'static str,
    role: &'static str,
    icon: &'static str,
    system_prompt: &'static str,
    capabilities: &'static [&'static str],
}

const DEFAULT_AGENTS: &[DefaultAgent] = &[
    DefaultAgent {
        name: "编程助手",
        role: "Developer",
        icon: "💻",
        system_prompt: "You are a professional developer assistant. Help the user with code review, debugging, and architecture design. Be concise and direct.",
        capabilities: &["web_search", "code_gen", "pdf_reader"],
    },
    DefaultAgent {
        name: "研究助手",
        role: "Researcher",
        icon: "🔍",
        system_prompt: "You are a research assistant. Help the user find information, analyze data, and write reports.",
        capabilities: &["web_search", "pdf_reader"],
    },
    DefaultAgent {
        name: "写作助手",
        role: "Writer",
        icon: "✍️",
        system_prompt: "You are a writing assistant. Help the user draft, edit, and polish content.",
        capabilities: &["translator"],
    },
    DefaultAgent {
        name: "数据分析",
        role: "Analyst",
        icon: "📊",
        system_prompt: "You are a data analyst. Help the user clean data, run analyses, and build visualizations.",
        capabilities: &["code_gen", "pdf_reader"],
    },
];

/// Ensure at least one provider and a set of starter Agents exist.
///
/// Idempotent: any existing rows short-circuit the seeding work.
pub async fn seed_defaults_if_empty(pool: &SqlitePool) -> Result<()> {
    let provider_id = ensure_default_provider(pool).await?;
    ensure_default_agents(pool, &provider_id).await?;
    Ok(())
}

async fn ensure_default_provider(pool: &SqlitePool) -> Result<ProviderId> {
    let existing = model_repo::list_providers(pool).await?;
    if let Some(p) = existing.iter().find(|p| p.is_default).or_else(|| existing.first()) {
        return Ok(p.id.clone());
    }

    let now = Utc::now();
    let provider = LlmProviderConfig {
        id: ProviderId::new(),
        name: "Stub (开发默认)".to_string(),
        provider_type: ProviderType::Custom,
        base_url: String::new(),
        model_name: "stub".to_string(),
        parameters: json!({}),
        is_default: true,
        created_at: now,
        updated_at: now,
    };
    let created = model_repo::create_provider(pool, &provider).await?;
    tracing::info!(provider_id = %created.id, "seeded default LLM provider");
    Ok(created.id)
}

async fn ensure_default_agents(pool: &SqlitePool, provider_id: &ProviderId) -> Result<()> {
    let existing = agent_repo::list_agents(pool).await?;
    if !existing.is_empty() {
        return Ok(());
    }

    for preset in DEFAULT_AGENTS {
        let now = Utc::now();
        let agent = AgentConfig {
            id: AgentId::new(),
            name: preset.name.to_string(),
            role: preset.role.to_string(),
            system_prompt: Some(preset.system_prompt.to_string()),
            model_id: provider_id.clone(),
            icon: Some(preset.icon.to_string()),
            status: AgentStatus::Idle,
            capabilities: preset.capabilities.iter().map(|s| s.to_string()).collect(),
            created_at: now,
            updated_at: now,
            last_active_at: None,
        };
        agent_repo::create_agent(pool, &agent).await?;
    }
    tracing::info!(count = DEFAULT_AGENTS.len(), "seeded default agents");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;

    #[tokio::test]
    async fn seeds_on_empty_db() {
        let db = Database::in_memory().await.unwrap();
        seed_defaults_if_empty(&db.main).await.unwrap();

        let providers = model_repo::list_providers(&db.main).await.unwrap();
        assert_eq!(providers.len(), 1);
        assert!(providers[0].is_default);

        let agents = agent_repo::list_agents(&db.main).await.unwrap();
        assert_eq!(agents.len(), DEFAULT_AGENTS.len());
        assert_eq!(agents[0].model_id, providers[0].id);
    }

    #[tokio::test]
    async fn seed_is_idempotent() {
        let db = Database::in_memory().await.unwrap();
        seed_defaults_if_empty(&db.main).await.unwrap();
        seed_defaults_if_empty(&db.main).await.unwrap();

        let agents = agent_repo::list_agents(&db.main).await.unwrap();
        assert_eq!(agents.len(), DEFAULT_AGENTS.len());
        let providers = model_repo::list_providers(&db.main).await.unwrap();
        assert_eq!(providers.len(), 1);
    }

    #[tokio::test]
    async fn skips_when_agents_exist() {
        let db = Database::in_memory().await.unwrap();

        // Pre-seed one provider and one agent, then verify seed() is a no-op.
        let pid = ensure_default_provider(&db.main).await.unwrap();
        let now = Utc::now();
        let existing = AgentConfig {
            id: AgentId::new(),
            name: "Existing".into(),
            role: "custom".into(),
            system_prompt: None,
            model_id: pid.clone(),
            icon: None,
            status: AgentStatus::Idle,
            capabilities: vec![],
            created_at: now,
            updated_at: now,
            last_active_at: None,
        };
        agent_repo::create_agent(&db.main, &existing).await.unwrap();

        seed_defaults_if_empty(&db.main).await.unwrap();
        let agents = agent_repo::list_agents(&db.main).await.unwrap();
        assert_eq!(agents.len(), 1);
    }
}
