//! ClawX Service — background daemon: composition root + API server.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use clawx_kb::EmbeddingService;
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

    let task_registry: Arc<dyn clawx_types::traits::TaskRegistryPort> =
        Arc::new(clawx_runtime::task_repo::SqliteTaskRegistry::new(db.main.clone()));
    let permission_gate: Arc<dyn clawx_types::traits::PermissionGatePort> =
        Arc::new(clawx_runtime::permission_repo::PermissionGate::new(db.main.clone()));

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
    )
    .with_task_registry(task_registry)
    .with_permission_gate(permission_gate);
    tracing::info!("runtime assembled with task registry + permission gate");

    // 6b. Initialize Embedding Service
    // Set EMBEDDING_LOCAL=true to use local candle-based inference (no server needed).
    // Otherwise, uses HTTP-based OpenAI-compatible API (TEI, vLLM, Ollama, etc.).
    let use_local_embedding = std::env::var("EMBEDDING_LOCAL")
        .map(|v| v == "true" || v == "1")
        .unwrap_or(false);

    let _embedding_service: Box<dyn clawx_kb::EmbeddingService> = if use_local_embedding {
        let local_config = clawx_kb::LocalEmbeddingConfig {
            model_id: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "Qwen/Qwen3-VL-Embedding-2B".to_string()),
            revision: std::env::var("EMBEDDING_REVISION")
                .unwrap_or_else(|_| "main".to_string()),
            use_gpu: std::env::var("EMBEDDING_USE_GPU")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(true),
            dimensions: std::env::var("EMBEDDING_DIMENSIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0),
            normalize: true,
            max_seq_len: std::env::var("EMBEDDING_MAX_SEQ_LEN")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(8192),
        };
        tracing::info!(
            model_id = %local_config.model_id,
            "loading local embedding service (candle)"
        );
        match clawx_kb::LocalEmbeddingService::load(local_config) {
            Ok(service) => {
                tracing::info!(
                    dimensions = service.dimensions(),
                    "local embedding service loaded"
                );
                Box::new(service)
            }
            Err(e) => {
                tracing::error!(
                    "failed to load local embedding model: {}. \
                     Falling back to HTTP embedding service.",
                    e
                );
                let fallback = clawx_kb::EmbeddingConfig::default();
                Box::new(clawx_kb::HttpEmbeddingService::new(fallback))
            }
        }
    } else {
        let embedding_config = clawx_kb::EmbeddingConfig {
            base_url: std::env::var("EMBEDDING_BASE_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            model_name: std::env::var("EMBEDDING_MODEL")
                .unwrap_or_else(|_| "Qwen/Qwen3-VL-Embedding-2B".to_string()),
            api_key: std::env::var("EMBEDDING_API_KEY").ok(),
            dimensions: std::env::var("EMBEDDING_DIMENSIONS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(1536),
            batch_size: 32,
        };
        tracing::info!(
            model = %embedding_config.model_name,
            base_url = %embedding_config.base_url,
            "HTTP embedding service configured"
        );
        Box::new(clawx_kb::HttpEmbeddingService::new(embedding_config))
    };

    // 6c. Initialize Reranker Service (Qwen3-VL-Reranker-2B via TEI)
    let reranker_config = clawx_kb::RerankerConfig {
        base_url: std::env::var("RERANKER_BASE_URL")
            .unwrap_or_else(|_| "http://localhost:8081".to_string()),
        model_name: std::env::var("RERANKER_MODEL")
            .unwrap_or_else(|_| "Qwen/Qwen3-VL-Reranker-2B".to_string()),
    };
    tracing::info!(
        model = %reranker_config.model_name,
        base_url = %reranker_config.base_url,
        "reranker service configured"
    );
    let _reranker_service = clawx_kb::HttpRerankerService::new(reranker_config);

    // 7. Initialize v0.2 components

    // Prompt Injection Guard (L1)
    let _injection_guard = clawx_security::prompt_defense::ClawxPromptInjectionGuard::new();
    tracing::info!("prompt injection guard initialized (L1)");

    // Run Recovery: detect and handle orphaned runs from previous service instance
    {
        let registry = clawx_runtime::task_repo::SqliteTaskRegistry::new(runtime.db.main.clone());
        let recovery_config = clawx_runtime::run_recovery::RunRecoveryConfig::default();
        match clawx_runtime::run_recovery::recover_orphaned_runs(&registry, &recovery_config).await {
            Ok(report) => {
                if report.orphaned_found > 0 {
                    tracing::info!(
                        orphaned = report.orphaned_found,
                        failed = report.marked_failed,
                        interrupted = report.marked_interrupted,
                        queued = report.left_queued,
                        retries = report.retries_scheduled,
                        "run recovery completed"
                    );
                } else {
                    tracing::info!("run recovery: no orphaned runs found");
                }
            }
            Err(e) => {
                tracing::warn!("run recovery failed (non-fatal): {}", e);
            }
        }
    }

    // Task Scheduler (background trigger scanning)
    let scheduler = clawx_scheduler::TaskScheduler::new(
        runtime.db.main.clone(),
        std::time::Duration::from_secs(30),
    );
    let _scheduler_handle = scheduler.start();
    tracing::info!("task scheduler started (30s scan interval)");

    // Channel Manager with adapter stubs
    let mut channel_manager = clawx_channel::ChannelManager::new();
    channel_manager.register_adapter(
        clawx_types::channel::ChannelType::Telegram,
        std::sync::Arc::new(clawx_channel::TelegramAdapter::new()),
    );
    channel_manager.register_adapter(
        clawx_types::channel::ChannelType::Lark,
        std::sync::Arc::new(clawx_channel::LarkAdapter::new()),
    );
    tracing::info!("channel manager initialized (Telegram + Lark adapters)");

    // Channel connection recovery: reconnect active channels from previous session
    {
        let channels = clawx_runtime::channel_repo::list_channels(&runtime.db.main).await;
        match channels {
            Ok(channels) => {
                let active: Vec<_> = channels.iter()
                    .filter(|c| c.status == clawx_types::channel::ChannelStatus::Connected)
                    .collect();
                if !active.is_empty() {
                    tracing::info!(count = active.len(), "reconnecting active channels");
                    for ch in active {
                        match channel_manager.connect(ch).await {
                            Ok(()) => tracing::info!(channel_id = %ch.id, "channel reconnected"),
                            Err(e) => tracing::warn!(channel_id = %ch.id, "channel reconnect failed: {}", e),
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("channel recovery failed (non-fatal): {}", e);
            }
        }
    }

    // 8. Build API router
    let state = clawx_api::AppState {
        runtime,
        control_token,
    };
    let router = clawx_api::build_router(state);

    // 9. Start server
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

    // ZhipuAI provider (if API key is set)
    if let Ok(api_key) = std::env::var("ZHIPU_API_KEY") {
        let base_url = std::env::var("ZHIPU_BASE_URL")
            .unwrap_or_else(|_| "https://open.bigmodel.cn/api/paas/v4".to_string());
        providers.insert(
            "zhipu".to_string(),
            Arc::new(clawx_llm::ZhipuProvider::new(api_key, base_url)),
        );
        tracing::info!("registered ZhipuAI LLM provider");
    }

    // Use first real provider as default, fall back to stub
    let default_provider = ["anthropic", "openai", "zhipu"]
        .iter()
        .find(|k| providers.contains_key(**k))
        .map(|k| k.to_string())
        .unwrap_or_else(|| "stub".to_string());
    tracing::info!(default = %default_provider, "LLM router default provider");

    Arc::new(clawx_llm::LlmRouter::new(providers, default_provider))
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
