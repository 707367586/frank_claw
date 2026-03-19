//! ClawX Service — background daemon: composition root + API server.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    tracing::info!("ClawX Service starting");

    // 1. Load configuration
    let config_loader = clawx_config::ConfigLoader::new(None).await?;
    let config = clawx_types::traits::ConfigService::load(&config_loader).await?;

    // 2. Ensure directory structure
    clawx_config::ConfigLoader::ensure_dirs(&config).await?;

    // 3. Write default config if missing
    config_loader.write_default_if_missing().await?;

    // 4. Generate control token
    let control_token = uuid::Uuid::new_v4().to_string();
    let token_path = format!("{}/run/control_token", config.storage.data_dir);
    let token_path = expand_tilde(&token_path);
    if let Some(parent) = token_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&token_path, &control_token).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(&token_path, perms).await?;
    }
    tracing::info!(path = %token_path.display(), "control token written");

    // 5. Initialize database
    let db = clawx_runtime::db::Database::init(&config).await?;
    tracing::info!("database initialized");

    // 6. Assemble runtime (composition root) with real implementations
    let llm = build_llm_router();
    let memory = Arc::new(clawx_memory::SqliteMemoryService::new(db.main.clone()));
    let vault = Arc::new(clawx_vault::SqliteVaultService::new(db.vault.clone()));
    let knowledge = {
        let tantivy_path = expand_tilde(&config.storage.tantivy_path);
        match clawx_kb::TantivyIndex::open(&tantivy_path) {
            Ok(tantivy) => {
                tracing::info!(path = %tantivy_path.display(), "Tantivy BM25 index opened");
                Arc::new(clawx_kb::SqliteKnowledgeService::with_tantivy(
                    db.main.clone(),
                    tantivy,
                ))
            }
            Err(e) => {
                tracing::warn!("Tantivy index unavailable, using SQLite LIKE fallback: {}", e);
                Arc::new(clawx_kb::SqliteKnowledgeService::new(db.main.clone()))
            }
        }
    };
    let security = build_security_guard(&config);

    let working_memory = Arc::new(clawx_memory::RealWorkingMemoryManager::new(
        memory.clone(),
        clawx_memory::WorkingMemoryConfig::default(),
    ));
    let memory_extractor: Arc<dyn clawx_types::traits::MemoryExtractor> =
        Arc::new(clawx_memory::LlmMemoryExtractor::new(
            llm.clone(),
            "default".to_string(),
        ));

    let runtime = clawx_runtime::Runtime::new(
        db,
        llm,
        memory,
        working_memory,
        memory_extractor,
        security,
        vault,
        knowledge,
        Arc::new(config_loader),
    );

    // 7. Build API router
    let state = clawx_api::AppState {
        runtime,
        control_token,
    };
    let router = clawx_api::build_router(state);

    // 8. Start server
    if let Some(port) = config.api.dev_port {
        tracing::info!(port, "starting in TCP dev mode");
        clawx_api::serve_tcp(router, port).await?;
    } else {
        let socket_path = expand_tilde(&config.api.socket_path);
        tracing::info!(path = %socket_path.display(), "starting on UDS");
        clawx_api::serve_uds(router, socket_path.to_str().unwrap()).await?;
    }

    Ok(())
}

/// Build the LLM router with providers from environment variables.
/// Falls back to StubLlmProvider when no API keys are configured.
fn build_llm_router() -> Arc<dyn clawx_types::traits::LlmProvider> {
    let mut providers: HashMap<String, Arc<dyn clawx_types::traits::LlmProvider>> = HashMap::new();

    // Always register the stub as fallback
    providers.insert(
        "stub".to_string(),
        Arc::new(clawx_llm::StubLlmProvider),
    );

    // Anthropic provider (if API key is set)
    if let Ok(api_key) = std::env::var("ANTHROPIC_API_KEY") {
        let base_url = std::env::var("ANTHROPIC_BASE_URL")
            .unwrap_or_else(|_| "https://api.anthropic.com".to_string());
        providers.insert(
            "anthropic".to_string(),
            Arc::new(clawx_llm::AnthropicProvider::new(api_key, base_url)),
        );
        tracing::info!("registered Anthropic LLM provider");
    }

    // OpenAI provider (if API key is set)
    if let Ok(api_key) = std::env::var("OPENAI_API_KEY") {
        let base_url = std::env::var("OPENAI_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com".to_string());
        providers.insert(
            "openai".to_string(),
            Arc::new(clawx_llm::OpenAiProvider::new(api_key, base_url)),
        );
        tracing::info!("registered OpenAI LLM provider");
    }

    Arc::new(clawx_llm::LlmRouter::new(providers, "stub".to_string()))
}

/// Build the security guard from config.
fn build_security_guard(
    config: &clawx_types::config::ClawxConfig,
) -> Arc<dyn clawx_types::traits::SecurityService> {
    let data_dir = expand_tilde(&config.storage.data_dir)
        .to_string_lossy()
        .to_string();
    let workspace_dir = format!("{}/workspace", data_dir);
    let allowed_dirs = vec![workspace_dir];

    if config.security.network_whitelist.is_empty() {
        Arc::new(clawx_security::ClawxSecurityGuard::new(allowed_dirs))
    } else {
        Arc::new(clawx_security::ClawxSecurityGuard::with_network_whitelist(
            allowed_dirs,
            config.security.network_whitelist.clone(),
        ))
    }
}

fn expand_tilde(path: &str) -> std::path::PathBuf {
    clawx_config::expand_tilde(path)
}
