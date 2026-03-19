use serde::{Deserialize, Serialize};

/// Top-level ClawX configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClawxConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub storage: StorageConfig,
    #[serde(default)]
    pub security: SecurityConfig,
    #[serde(default)]
    pub api: ApiConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_language")]
    pub language: String,
    #[serde(default = "default_theme")]
    pub theme: String,
    #[serde(default = "default_max_active_agents")]
    pub max_active_agents: u32,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: default_language(),
            theme: default_theme(),
            max_active_agents: default_max_active_agents(),
        }
    }
}

fn default_language() -> String {
    "en".to_string()
}
fn default_theme() -> String {
    "system".to_string()
}
fn default_max_active_agents() -> u32 {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageConfig {
    /// Root directory for all ClawX data.
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    /// Qdrant storage path.
    #[serde(default = "default_qdrant_path")]
    pub qdrant_path: String,
    /// Tantivy index path.
    #[serde(default = "default_tantivy_path")]
    pub tantivy_path: String,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            data_dir: default_data_dir(),
            qdrant_path: default_qdrant_path(),
            tantivy_path: default_tantivy_path(),
        }
    }
}

fn default_data_dir() -> String {
    "~/.clawx".to_string()
}
fn default_qdrant_path() -> String {
    "~/.clawx/qdrant".to_string()
}
fn default_tantivy_path() -> String {
    "~/.clawx/tantivy".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    #[serde(default)]
    pub network_whitelist: Vec<String>,
    #[serde(default = "default_rate_limit")]
    pub rate_limit_per_minute: u32,
    #[serde(default = "default_audit_dir")]
    pub audit_dir: String,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            network_whitelist: Vec::new(),
            rate_limit_per_minute: default_rate_limit(),
            audit_dir: default_audit_dir(),
        }
    }
}

fn default_rate_limit() -> u32 {
    60
}
fn default_audit_dir() -> String {
    "~/.clawx/audit".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default = "default_socket_path")]
    pub socket_path: String,
    /// Optional TCP port for development mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dev_port: Option<u16>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            socket_path: default_socket_path(),
            dev_port: None,
        }
    }
}

fn default_socket_path() -> String {
    "~/.clawx/run/clawx.sock".to_string()
}
