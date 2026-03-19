//! TOML configuration loader with directory initialization.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use clawx_types::config::ClawxConfig;
use clawx_types::error::Result;
use clawx_types::traits::ConfigService;
use tokio::sync::RwLock;
use tracing::{debug, info};

/// Resolves `~` to the user's home directory.
pub fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Configuration loader that reads TOML files and initializes directory structure.
#[derive(Debug)]
pub struct ConfigLoader {
    config_path: PathBuf,
    config: RwLock<ClawxConfig>,
}

impl ConfigLoader {
    /// Create a new ConfigLoader.
    ///
    /// If `config_path` is `None`, defaults to `~/.clawx/config.toml`.
    pub async fn new(config_path: Option<PathBuf>) -> Result<Self> {
        let path = config_path.unwrap_or_else(|| expand_tilde("~/.clawx/config.toml"));
        let config = Self::load_from_path(&path).await?;
        let loader = Self {
            config_path: path,
            config: RwLock::new(config),
        };
        Ok(loader)
    }

    /// Create a ConfigLoader with default config (no file read).
    /// Useful for testing.
    pub fn with_defaults() -> Self {
        Self {
            config_path: PathBuf::from("/dev/null"),
            config: RwLock::new(ClawxConfig::default()),
        }
    }

    /// Initialize the `~/.clawx/` directory tree.
    pub async fn ensure_dirs(config: &ClawxConfig) -> Result<()> {
        let data_dir = expand_tilde(&config.storage.data_dir);
        let dirs = [
            data_dir.clone(),
            data_dir.join("db"),
            data_dir.join("vault"),
            data_dir.join("run"),
            expand_tilde(&config.storage.qdrant_path),
            expand_tilde(&config.storage.tantivy_path),
            expand_tilde(&config.security.audit_dir),
        ];
        for dir in &dirs {
            if !dir.exists() {
                tokio::fs::create_dir_all(dir).await.map_err(|e| {
                    clawx_types::error::ClawxError::Internal(format!(
                        "failed to create directory {}: {}",
                        dir.display(),
                        e
                    ))
                })?;
                info!(path = %dir.display(), "created directory");
            }
        }
        Ok(())
    }

    /// Load config from a TOML file. Returns default config if file doesn't exist.
    async fn load_from_path(path: &Path) -> Result<ClawxConfig> {
        if path.exists() {
            let content = tokio::fs::read_to_string(path).await.map_err(|e| {
                clawx_types::error::ClawxError::Internal(format!(
                    "failed to read config {}: {}",
                    path.display(),
                    e
                ))
            })?;
            let config: ClawxConfig = toml::from_str(&content).map_err(|e| {
                clawx_types::error::ClawxError::Internal(format!("invalid config TOML: {}", e))
            })?;
            debug!(path = %path.display(), "loaded config from file");
            Ok(config)
        } else {
            debug!(path = %path.display(), "config file not found, using defaults");
            Ok(ClawxConfig::default())
        }
    }

    /// Write the default config to disk if it doesn't exist.
    pub async fn write_default_if_missing(&self) -> Result<()> {
        if !self.config_path.exists() {
            if let Some(parent) = self.config_path.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    clawx_types::error::ClawxError::Internal(format!(
                        "failed to create config dir: {}",
                        e
                    ))
                })?;
            }
            let config = self.config.read().await;
            let content = toml::to_string_pretty(&*config).map_err(|e| {
                clawx_types::error::ClawxError::Internal(format!(
                    "failed to serialize config: {}",
                    e
                ))
            })?;
            tokio::fs::write(&self.config_path, content).await.map_err(|e| {
                clawx_types::error::ClawxError::Internal(format!(
                    "failed to write config: {}",
                    e
                ))
            })?;
            info!(path = %self.config_path.display(), "wrote default config");
        }
        Ok(())
    }
}

#[async_trait]
impl ConfigService for ConfigLoader {
    async fn load(&self) -> Result<ClawxConfig> {
        Ok(self.config.read().await.clone())
    }

    async fn reload(&self) -> Result<ClawxConfig> {
        let new_config = Self::load_from_path(&self.config_path).await?;
        let mut config = self.config.write().await;
        *config = new_config.clone();
        info!("config reloaded");
        Ok(new_config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn default_config_has_sane_values() {
        let loader = ConfigLoader::with_defaults();
        let config = loader.load().await.unwrap();
        assert_eq!(config.general.language, "en");
        assert_eq!(config.general.max_active_agents, 3);
        assert_eq!(config.storage.data_dir, "~/.clawx");
        assert!(config.api.dev_port.is_none());
    }

    #[tokio::test]
    async fn load_from_toml_file() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        let toml_content = r#"
[general]
language = "zh"
max_active_agents = 5

[api]
dev_port = 8080
"#;
        tokio::fs::write(&config_path, toml_content).await.unwrap();

        let loader = ConfigLoader::new(Some(config_path)).await.unwrap();
        let config = loader.load().await.unwrap();
        assert_eq!(config.general.language, "zh");
        assert_eq!(config.general.max_active_agents, 5);
        assert_eq!(config.api.dev_port, Some(8080));
    }

    #[tokio::test]
    async fn missing_file_returns_defaults() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("nonexistent.toml");

        let loader = ConfigLoader::new(Some(config_path)).await.unwrap();
        let config = loader.load().await.unwrap();
        assert_eq!(config.general.language, "en");
    }

    #[tokio::test]
    async fn ensure_dirs_creates_directory_tree() {
        let tmp = TempDir::new().unwrap();
        let data_dir = tmp.path().join("clawx");
        let config = ClawxConfig {
            storage: clawx_types::config::StorageConfig {
                data_dir: data_dir.to_string_lossy().to_string(),
                qdrant_path: data_dir.join("qdrant").to_string_lossy().to_string(),
                tantivy_path: data_dir.join("tantivy").to_string_lossy().to_string(),
            },
            security: clawx_types::config::SecurityConfig {
                audit_dir: data_dir.join("audit").to_string_lossy().to_string(),
                ..Default::default()
            },
            ..Default::default()
        };

        ConfigLoader::ensure_dirs(&config).await.unwrap();

        assert!(data_dir.exists());
        assert!(data_dir.join("db").exists());
        assert!(data_dir.join("vault").exists());
        assert!(data_dir.join("run").exists());
        assert!(data_dir.join("qdrant").exists());
        assert!(data_dir.join("tantivy").exists());
        assert!(data_dir.join("audit").exists());
    }

    #[tokio::test]
    async fn reload_picks_up_changes() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("config.toml");
        let toml_v1 = r#"
[general]
language = "en"
"#;
        tokio::fs::write(&config_path, toml_v1).await.unwrap();

        let loader = ConfigLoader::new(Some(config_path.clone())).await.unwrap();
        let c1 = loader.load().await.unwrap();
        assert_eq!(c1.general.language, "en");

        let toml_v2 = r#"
[general]
language = "zh"
"#;
        tokio::fs::write(&config_path, toml_v2).await.unwrap();

        let c2 = loader.reload().await.unwrap();
        assert_eq!(c2.general.language, "zh");
    }

    #[tokio::test]
    async fn write_default_if_missing_creates_file() {
        let tmp = TempDir::new().unwrap();
        let config_path = tmp.path().join("sub/config.toml");

        let loader = ConfigLoader::new(Some(config_path.clone())).await.unwrap();
        assert!(!config_path.exists());

        loader.write_default_if_missing().await.unwrap();
        assert!(config_path.exists());

        // Verify the written file is valid TOML
        let content = tokio::fs::read_to_string(&config_path).await.unwrap();
        let _: ClawxConfig = toml::from_str(&content).unwrap();
    }

    #[test]
    fn config_service_is_object_safe() {
        fn _assert(_: std::sync::Arc<dyn ConfigService>) {}
    }
}
