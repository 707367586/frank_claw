use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, ConversationId, MessageId, ProviderId};
use crate::llm::TokenUsage;

/// Current operational status of an agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Idle,
    Active,
    Error,
    Offline,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Active => write!(f, "active"),
            Self::Error => write!(f, "error"),
            Self::Offline => write!(f, "offline"),
        }
    }
}

impl std::str::FromStr for AgentStatus {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "idle" => Ok(Self::Idle),
            "active" => Ok(Self::Active),
            "error" => Ok(Self::Error),
            "offline" => Ok(Self::Offline),
            other => Err(format!("unknown agent status: {}", other)),
        }
    }
}

/// Persistent configuration for an agent instance.
/// Aligned with `agents` table in data-model.md §2.1.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: AgentId,
    pub name: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system_prompt: Option<String>,
    /// ID of the bound LLM provider.
    pub model_id: ProviderId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub status: AgentStatus,
    /// Enabled capability presets (JSON array).
    #[serde(default)]
    pub capabilities: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_active_at: Option<DateTime<Utc>>,
}

/// A conversation belonging to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: ConversationId,
    pub agent_id: AgentId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub status: ConversationStatus,
    pub messages: Vec<ConversationMessage>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConversationStatus {
    Active,
    Archived,
    Deleted,
}

/// A message within a conversation, with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMessage {
    pub id: MessageId,
    pub conversation_id: ConversationId,
    pub role: crate::llm::MessageRole,
    pub content: String,
    /// JSON metadata: tool_calls, citations, etc.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

/// A message sent by the user to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserMessage {
    pub content: String,
    #[serde(default)]
    pub attachments: Vec<String>,
}

/// The response produced by an agent after processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub content: String,
    pub tool_calls_made: u32,
    pub tokens_used: TokenUsage,
}

/// Template for creating an agent from a preset.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTemplate {
    pub name: String,
    pub role: String,
    pub system_prompt: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}
