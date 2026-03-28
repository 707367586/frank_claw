use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::SkillId;

/// Status of an installed skill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillStatus {
    Enabled,
    Disabled,
}

impl Default for SkillStatus {
    fn default() -> Self {
        Self::Enabled
    }
}

impl std::fmt::Display for SkillStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Enabled => write!(f, "enabled"),
            Self::Disabled => write!(f, "disabled"),
        }
    }
}

impl std::str::FromStr for SkillStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "enabled" => Ok(Self::Enabled),
            "disabled" => Ok(Self::Disabled),
            other => Err(format!("unknown skill status: {}", other)),
        }
    }
}

/// Capability declaration in a skill manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDeclaration {
    #[serde(default)]
    pub net_http: Vec<String>,
    #[serde(default)]
    pub secrets: Vec<String>,
    #[serde(default)]
    pub fs_read: Vec<String>,
    #[serde(default)]
    pub fs_write: Vec<String>,
    #[serde(default)]
    pub exec_shell: Vec<String>,
}

impl Default for CapabilityDeclaration {
    fn default() -> Self {
        Self {
            net_http: Vec::new(),
            secrets: Vec::new(),
            fs_read: Vec::new(),
            fs_write: Vec::new(),
            exec_shell: Vec::new(),
        }
    }
}

/// Skill manifest (parsed from capabilities.toml in the skill package).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    pub name: String,
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    pub entrypoint: String,
    #[serde(default)]
    pub capabilities: CapabilityDeclaration,
}

/// An installed skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: SkillId,
    pub name: String,
    pub version: String,
    pub manifest: SkillManifest,
    pub status: SkillStatus,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
