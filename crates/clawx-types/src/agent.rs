use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::AgentId;
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

/// Persistent configuration for an agent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    pub id: AgentId,
    pub name: String,
    pub role: String,
    pub system_prompt: String,
    pub model_provider: String,
    pub model_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_params: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
