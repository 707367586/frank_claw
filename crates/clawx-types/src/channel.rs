use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentId, ChannelId};

/// Supported IM channel types.
/// Aligned with PRD §2.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelType {
    Lark,
    Telegram,
    Slack,
    #[serde(rename = "whatsapp")]
    WhatsApp,
    Discord,
    #[serde(rename = "wecom")]
    WeCom,
}

impl std::fmt::Display for ChannelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Lark => write!(f, "lark"),
            Self::Telegram => write!(f, "telegram"),
            Self::Slack => write!(f, "slack"),
            Self::WhatsApp => write!(f, "whatsapp"),
            Self::Discord => write!(f, "discord"),
            Self::WeCom => write!(f, "wecom"),
        }
    }
}

impl std::str::FromStr for ChannelType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "lark" => Ok(Self::Lark),
            "telegram" => Ok(Self::Telegram),
            "slack" => Ok(Self::Slack),
            "whatsapp" => Ok(Self::WhatsApp),
            "discord" => Ok(Self::Discord),
            "wecom" => Ok(Self::WeCom),
            other => Err(format!("unknown channel type: {}", other)),
        }
    }
}

/// Connection status of a channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelStatus {
    Connected,
    Disconnected,
    Error,
}

impl Default for ChannelStatus {
    fn default() -> Self {
        Self::Disconnected
    }
}

impl std::fmt::Display for ChannelStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connected => write!(f, "connected"),
            Self::Disconnected => write!(f, "disconnected"),
            Self::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for ChannelStatus {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "connected" => Ok(Self::Connected),
            "disconnected" => Ok(Self::Disconnected),
            "error" => Ok(Self::Error),
            other => Err(format!("unknown channel status: {}", other)),
        }
    }
}

/// A configured IM channel.
/// Aligned with `channels` table in data-model.md §2.7.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub channel_type: ChannelType,
    pub name: String,
    /// JSON: channel-specific config (token, webhook URL, etc.)
    pub config: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    pub status: ChannelStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// An inbound message from an IM channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    pub channel_id: ChannelId,
    pub sender_id: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub is_direct_message: bool,
    pub received_at: DateTime<Utc>,
}

/// An outbound message to be sent via an IM channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    pub channel_id: ChannelId,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
}
