//! Telegram Bot API adapter for ClawX channels.
//!
//! Implements the `ChannelAdapter` trait using the Telegram Bot HTTP API
//! with long-polling for inbound messages.

use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use clawx_types::channel::{Channel, OutboundMessage};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::ChannelId;

use crate::ChannelAdapter;

const TELEGRAM_API_BASE: &str = "https://api.telegram.org/bot";

// ---------------------------------------------------------------------------
// Telegram API types (private)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct TelegramResponse<T> {
    ok: bool,
    #[serde(default)]
    description: Option<String>,
    result: Option<T>,
}

#[derive(Debug, Deserialize)]
struct TelegramUser {
    #[allow(dead_code)]
    id: i64,
    #[allow(dead_code)]
    is_bot: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TelegramUpdate {
    pub update_id: i64,
    pub message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TelegramMessage {
    pub chat: TelegramChat,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub from: Option<TelegramFrom>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TelegramChat {
    pub id: i64,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TelegramFrom {
    pub id: i64,
}

#[derive(Debug, Serialize)]
struct SendMessageRequest<'a> {
    chat_id: i64,
    text: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    parse_mode: Option<&'a str>,
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

struct TelegramConnection {
    bot_token: String,
    default_chat_id: Option<i64>,
    poll_handle: Option<tokio::task::JoinHandle<()>>,
}

// ---------------------------------------------------------------------------
// TelegramAdapter
// ---------------------------------------------------------------------------

/// Adapter for the Telegram Bot API using long polling.
pub struct TelegramAdapter {
    client: reqwest::Client,
    connections: RwLock<HashMap<ChannelId, TelegramConnection>>,
}

impl TelegramAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Create an adapter with a custom `reqwest::Client` (useful for testing).
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client,
            connections: RwLock::new(HashMap::new()),
        }
    }

    /// Build the API URL for a given token and method.
    fn api_url(token: &str, method: &str) -> String {
        format!("{}{}/{}", TELEGRAM_API_BASE, token, method)
    }

    /// Verify the bot token by calling `getMe`.
    async fn verify_token(&self, token: &str) -> Result<()> {
        let url = Self::api_url(token, "getMe");
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClawxError::Channel(format!("telegram getMe request failed: {e}")))?;

        let body: TelegramResponse<TelegramUser> = resp
            .json()
            .await
            .map_err(|e| ClawxError::Channel(format!("telegram getMe parse failed: {e}")))?;

        if !body.ok {
            return Err(ClawxError::Channel(format!(
                "telegram getMe returned error: {}",
                body.description.unwrap_or_default()
            )));
        }

        Ok(())
    }
}

impl Default for TelegramAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelAdapter for TelegramAdapter {
    async fn connect(&self, channel: &Channel) -> Result<()> {
        let bot_token = channel
            .config
            .get("bot_token")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ClawxError::Channel(
                    "telegram channel requires 'bot_token' in config".to_string(),
                )
            })?
            .to_string();

        // Validate token format: should be `<number>:<alphanumeric>`
        if !bot_token.contains(':') {
            return Err(ClawxError::Channel(
                "telegram bot_token has invalid format (expected <id>:<hash>)".to_string(),
            ));
        }

        // Verify token against Telegram API
        self.verify_token(&bot_token).await?;

        let default_chat_id = channel
            .config
            .get("chat_id")
            .and_then(|v| v.as_i64());

        let conn = TelegramConnection {
            bot_token,
            default_chat_id,
            poll_handle: None,
        };

        let mut connections = self.connections.write().await;
        connections.insert(channel.id, conn);

        tracing::info!(channel_id = %channel.id, "telegram adapter: connected");
        Ok(())
    }

    async fn disconnect(&self, channel_id: &ChannelId) -> Result<()> {
        let mut connections = self.connections.write().await;
        if let Some(conn) = connections.remove(channel_id) {
            if let Some(handle) = conn.poll_handle {
                handle.abort();
            }
            tracing::info!(channel_id = %channel_id, "telegram adapter: disconnected");
        }
        Ok(())
    }

    async fn send_message(&self, msg: &OutboundMessage) -> Result<()> {
        let connections = self.connections.read().await;
        let conn = connections.get(&msg.channel_id).ok_or_else(|| {
            ClawxError::Channel(format!(
                "telegram channel {} is not connected",
                msg.channel_id
            ))
        })?;

        // Determine chat_id: prefer reply_to (which carries the Telegram chat_id),
        // fall back to the default_chat_id from config.
        let chat_id: i64 = if let Some(ref reply_to) = msg.reply_to {
            reply_to.parse::<i64>().map_err(|_| {
                ClawxError::Channel(format!(
                    "telegram reply_to '{}' is not a valid chat_id",
                    reply_to
                ))
            })?
        } else if let Some(default) = conn.default_chat_id {
            default
        } else {
            return Err(ClawxError::Channel(
                "telegram send_message requires a chat_id via reply_to or default_chat_id"
                    .to_string(),
            ));
        };

        let body = SendMessageRequest {
            chat_id,
            text: &msg.content,
            parse_mode: Some("Markdown"),
        };

        let url = Self::api_url(&conn.bot_token, "sendMessage");
        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::Channel(format!("telegram sendMessage request failed: {e}")))?;

        let status = resp.status();
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(ClawxError::Channel(
                "telegram rate limited (429), retry later".to_string(),
            ));
        }

        let api_resp: TelegramResponse<serde_json::Value> = resp
            .json()
            .await
            .map_err(|e| ClawxError::Channel(format!("telegram sendMessage parse failed: {e}")))?;

        if !api_resp.ok {
            return Err(ClawxError::Channel(format!(
                "telegram sendMessage error: {}",
                api_resp.description.unwrap_or_default()
            )));
        }

        tracing::debug!(
            channel_id = %msg.channel_id,
            chat_id = chat_id,
            "telegram adapter: message sent"
        );

        Ok(())
    }

    async fn is_connected(&self, channel_id: &ChannelId) -> bool {
        let connections = self.connections.read().await;
        connections.contains_key(channel_id)
    }
}

// ---------------------------------------------------------------------------
// Inbound polling (not part of ChannelAdapter trait)
// ---------------------------------------------------------------------------

/// Represents an inbound message received from Telegram.
#[derive(Debug, Clone)]
pub struct InboundTelegramMessage {
    pub channel_id: ChannelId,
    pub chat_id: i64,
    pub sender_id: i64,
    pub text: String,
    pub update_id: i64,
}

impl TelegramAdapter {
    /// Poll for updates once using long polling. Returns parsed inbound messages.
    /// `offset` should be the last processed `update_id + 1` to avoid duplicates.
    pub async fn poll_updates(
        &self,
        channel_id: &ChannelId,
        offset: Option<i64>,
    ) -> Result<(Vec<InboundTelegramMessage>, Option<i64>)> {
        let connections = self.connections.read().await;
        let conn = connections.get(channel_id).ok_or_else(|| {
            ClawxError::Channel(format!(
                "telegram channel {} is not connected",
                channel_id
            ))
        })?;

        let mut url = Self::api_url(&conn.bot_token, "getUpdates");
        url.push_str("?timeout=30");
        if let Some(off) = offset {
            url.push_str(&format!("&offset={off}"));
        }

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| ClawxError::Channel(format!("telegram getUpdates failed: {e}")))?;

        let body: TelegramResponse<Vec<TelegramUpdate>> = resp
            .json()
            .await
            .map_err(|e| ClawxError::Channel(format!("telegram getUpdates parse failed: {e}")))?;

        if !body.ok {
            return Err(ClawxError::Channel(format!(
                "telegram getUpdates error: {}",
                body.description.unwrap_or_default()
            )));
        }

        let updates = body.result.unwrap_or_default();
        let mut messages = Vec::new();
        let mut new_offset: Option<i64> = offset;

        for update in &updates {
            // Track the highest update_id so the next poll can skip processed updates.
            let next = update.update_id + 1;
            new_offset = Some(new_offset.map_or(next, |o| o.max(next)));

            if let Some(ref tg_msg) = update.message {
                if let Some(ref text) = tg_msg.text {
                    messages.push(InboundTelegramMessage {
                        channel_id: *channel_id,
                        chat_id: tg_msg.chat.id,
                        sender_id: tg_msg.from.as_ref().map_or(0, |f| f.id),
                        text: text.clone(),
                        update_id: update.update_id,
                    });
                }
            }
        }

        Ok((messages, new_offset))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use clawx_types::channel::ChannelStatus;

    fn make_channel(config: serde_json::Value) -> Channel {
        let now = Utc::now();
        Channel {
            id: ChannelId::new(),
            channel_type: clawx_types::channel::ChannelType::Telegram,
            name: "test-tg".to_string(),
            config,
            agent_id: None,
            status: ChannelStatus::Disconnected,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn telegram_connect_validates_token_exists() {
        let adapter = TelegramAdapter::new();
        let channel = make_channel(serde_json::json!({}));

        let result = adapter.connect(&channel).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("bot_token")),
            _ => panic!("expected Channel error about bot_token"),
        }
    }

    #[tokio::test]
    async fn telegram_connect_validates_token_format() {
        let adapter = TelegramAdapter::new();
        let channel = make_channel(serde_json::json!({"bot_token": "invalid-no-colon"}));

        let result = adapter.connect(&channel).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("invalid format")),
            _ => panic!("expected Channel error about format"),
        }
    }

    #[tokio::test]
    async fn telegram_send_message_requires_connection() {
        let adapter = TelegramAdapter::new();
        let msg = OutboundMessage {
            channel_id: ChannelId::new(),
            content: "hello".to_string(),
            thread_id: None,
            reply_to: Some("12345".to_string()),
        };

        let result = adapter.send_message(&msg).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("not connected")),
            _ => panic!("expected Channel error about connection"),
        }
    }

    #[tokio::test]
    async fn telegram_disconnect_clears_state() {
        // We can't fully connect (needs real API), so manually insert a connection.
        let adapter = TelegramAdapter::new();
        let channel_id = ChannelId::new();

        {
            let mut connections = adapter.connections.write().await;
            connections.insert(
                channel_id,
                TelegramConnection {
                    bot_token: "123:ABC".to_string(),
                    default_chat_id: None,
                    poll_handle: None,
                },
            );
        }

        assert!(adapter.is_connected(&channel_id).await);

        adapter.disconnect(&channel_id).await.unwrap();
        assert!(!adapter.is_connected(&channel_id).await);
    }

    #[tokio::test]
    async fn telegram_send_requires_chat_id() {
        let adapter = TelegramAdapter::new();
        let channel_id = ChannelId::new();

        // Insert a connection with no default_chat_id
        {
            let mut connections = adapter.connections.write().await;
            connections.insert(
                channel_id,
                TelegramConnection {
                    bot_token: "123:ABC".to_string(),
                    default_chat_id: None,
                    poll_handle: None,
                },
            );
        }

        let msg = OutboundMessage {
            channel_id,
            content: "hello".to_string(),
            thread_id: None,
            reply_to: None, // no chat_id provided
        };

        let result = adapter.send_message(&msg).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("chat_id")),
            _ => panic!("expected Channel error about chat_id"),
        }
    }

    /// Integration test that requires a real Telegram bot token.
    /// Run with: TELEGRAM_BOT_TOKEN=<token> TELEGRAM_CHAT_ID=<id> cargo test -p clawx-channel telegram_real -- --ignored
    #[tokio::test]
    #[ignore]
    async fn telegram_real_connect_and_send() {
        let token = std::env::var("TELEGRAM_BOT_TOKEN").expect("set TELEGRAM_BOT_TOKEN");
        let chat_id: i64 = std::env::var("TELEGRAM_CHAT_ID")
            .expect("set TELEGRAM_CHAT_ID")
            .parse()
            .expect("TELEGRAM_CHAT_ID must be i64");

        let adapter = TelegramAdapter::new();
        let channel = make_channel(serde_json::json!({
            "bot_token": token,
            "chat_id": chat_id,
        }));

        adapter.connect(&channel).await.unwrap();
        assert!(adapter.is_connected(&channel.id).await);

        let msg = OutboundMessage {
            channel_id: channel.id,
            content: "Hello from ClawX test!".to_string(),
            thread_id: None,
            reply_to: None,
        };
        adapter.send_message(&msg).await.unwrap();

        adapter.disconnect(&channel.id).await.unwrap();
        assert!(!adapter.is_connected(&channel.id).await);
    }
}
