//! IM channel adapters and manager for ClawX v0.2.
//!
//! Provides the ChannelAdapter trait for messaging platform integrations,
//! a ChannelManager for lifecycle management, and platform adapters for
//! Telegram (Bot API) and Lark/Feishu (Open API).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use clawx_types::channel::{Channel, ChannelType, OutboundMessage};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::ChannelId;

pub mod lark;
pub mod telegram;

pub use lark::LarkAdapter;
pub use telegram::TelegramAdapter;

/// Adapter trait for connecting to a messaging platform.
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Connect to the channel.
    async fn connect(&self, channel: &Channel) -> Result<()>;

    /// Disconnect from the channel.
    async fn disconnect(&self, channel_id: &ChannelId) -> Result<()>;

    /// Send a message through this channel.
    async fn send_message(&self, msg: &OutboundMessage) -> Result<()>;

    /// Check connection health.
    async fn is_connected(&self, channel_id: &ChannelId) -> bool;
}

/// Channel manager that manages all channel connections.
pub struct ChannelManager {
    adapters: HashMap<ChannelType, Arc<dyn ChannelAdapter>>,
    connected: RwLock<HashMap<ChannelId, ChannelType>>,
}

impl ChannelManager {
    pub fn new() -> Self {
        Self {
            adapters: HashMap::new(),
            connected: RwLock::new(HashMap::new()),
        }
    }

    /// Register an adapter for a channel type.
    pub fn register_adapter(
        &mut self,
        channel_type: ChannelType,
        adapter: Arc<dyn ChannelAdapter>,
    ) {
        self.adapters.insert(channel_type, adapter);
    }

    /// Connect a channel.
    pub async fn connect(&self, channel: &Channel) -> Result<()> {
        let adapter = self
            .adapters
            .get(&channel.channel_type)
            .ok_or_else(|| {
                ClawxError::Channel(format!(
                    "no adapter registered for channel type: {}",
                    channel.channel_type
                ))
            })?;

        adapter.connect(channel).await?;

        let mut connected = self.connected.write().await;
        connected.insert(channel.id, channel.channel_type);

        Ok(())
    }

    /// Disconnect a channel.
    pub async fn disconnect(&self, channel_id: &ChannelId, channel_type: ChannelType) -> Result<()> {
        let adapter = self
            .adapters
            .get(&channel_type)
            .ok_or_else(|| {
                ClawxError::Channel(format!(
                    "no adapter registered for channel type: {}",
                    channel_type
                ))
            })?;

        adapter.disconnect(channel_id).await?;

        let mut connected = self.connected.write().await;
        connected.remove(channel_id);

        Ok(())
    }

    /// Send a message through a connected channel.
    pub async fn send_message(
        &self,
        channel_type: ChannelType,
        msg: &OutboundMessage,
    ) -> Result<()> {
        let adapter = self
            .adapters
            .get(&channel_type)
            .ok_or_else(|| {
                ClawxError::Channel(format!(
                    "no adapter registered for channel type: {}",
                    channel_type
                ))
            })?;

        adapter.send_message(msg).await
    }

    /// Get all currently connected channel IDs.
    pub async fn connected_channels(&self) -> Vec<ChannelId> {
        let connected = self.connected.read().await;
        connected.keys().copied().collect()
    }

    /// Check if a channel is connected.
    pub async fn is_connected(&self, channel_id: &ChannelId) -> bool {
        let connected = self.connected.read().await;
        connected.contains_key(channel_id)
    }
}

impl Default for ChannelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Stub adapters for testing and initial development
// ---------------------------------------------------------------------------

/// Stub adapter that logs operations (for testing).
pub struct StubChannelAdapter {
    channel_type: ChannelType,
    connected: RwLock<HashMap<ChannelId, bool>>,
}

impl StubChannelAdapter {
    pub fn new(channel_type: ChannelType) -> Self {
        Self {
            channel_type,
            connected: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl ChannelAdapter for StubChannelAdapter {
    async fn connect(&self, channel: &Channel) -> Result<()> {
        tracing::info!(
            channel_id = %channel.id,
            channel_type = %self.channel_type,
            "stub adapter: connect"
        );
        let mut connected = self.connected.write().await;
        connected.insert(channel.id, true);
        Ok(())
    }

    async fn disconnect(&self, channel_id: &ChannelId) -> Result<()> {
        tracing::info!(
            channel_id = %channel_id,
            channel_type = %self.channel_type,
            "stub adapter: disconnect"
        );
        let mut connected = self.connected.write().await;
        connected.remove(channel_id);
        Ok(())
    }

    async fn send_message(&self, msg: &OutboundMessage) -> Result<()> {
        tracing::info!(
            channel_id = %msg.channel_id,
            content_len = msg.content.len(),
            "stub adapter: send_message"
        );
        Ok(())
    }

    async fn is_connected(&self, channel_id: &ChannelId) -> bool {
        let connected = self.connected.read().await;
        connected.contains_key(channel_id)
    }
}

// ---------------------------------------------------------------------------
// MessageRouter: routes inbound messages to the correct agent
// ---------------------------------------------------------------------------

/// Inbound message from a channel, ready for routing.
#[derive(Debug, Clone)]
pub struct RoutedMessage {
    pub channel_id: ChannelId,
    pub agent_id: clawx_types::ids::AgentId,
    pub content: String,
    pub sender_id: String,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// MessageRouter: routes inbound messages to the correct agent based on channel binding.
pub struct MessageRouter {
    #[allow(dead_code)]
    channel_manager: Arc<ChannelManager>,
}

impl MessageRouter {
    pub fn new(channel_manager: Arc<ChannelManager>) -> Self {
        Self { channel_manager }
    }

    /// Route an inbound message to the correct agent based on channel binding.
    /// Returns `None` if the channel has no bound agent.
    pub fn route(
        &self,
        channel: &Channel,
        sender_id: &str,
        content: &str,
    ) -> Option<RoutedMessage> {
        let agent_id = channel.agent_id?;

        Some(RoutedMessage {
            channel_id: channel.id,
            agent_id,
            content: content.to_string(),
            sender_id: sender_id.to_string(),
            timestamp: chrono::Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use clawx_types::channel::ChannelStatus;

    fn make_channel(channel_type: ChannelType, config: serde_json::Value) -> Channel {
        let now = Utc::now();
        Channel {
            id: ChannelId::new(),
            channel_type,
            name: "test-channel".to_string(),
            config,
            agent_id: None,
            status: ChannelStatus::Disconnected,
            created_at: now,
            updated_at: now,
        }
    }

    fn make_outbound(channel_id: ChannelId) -> OutboundMessage {
        OutboundMessage {
            channel_id,
            content: "Hello from agent".to_string(),
            thread_id: None,
            reply_to: None,
        }
    }

    // -----------------------------------------------------------------------
    // StubChannelAdapter
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn stub_adapter_connect_disconnect() {
        let adapter = StubChannelAdapter::new(ChannelType::Telegram);
        let channel = make_channel(ChannelType::Telegram, serde_json::json!({}));

        adapter.connect(&channel).await.unwrap();
        assert!(adapter.is_connected(&channel.id).await);

        adapter.disconnect(&channel.id).await.unwrap();
        assert!(!adapter.is_connected(&channel.id).await);
    }

    #[tokio::test]
    async fn stub_adapter_send_message() {
        let adapter = StubChannelAdapter::new(ChannelType::Telegram);
        let channel = make_channel(ChannelType::Telegram, serde_json::json!({}));
        let msg = make_outbound(channel.id);

        adapter.connect(&channel).await.unwrap();
        adapter.send_message(&msg).await.unwrap();
    }

    // -----------------------------------------------------------------------
    // ChannelManager
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn manager_connect_and_disconnect() {
        let mut manager = ChannelManager::new();
        manager.register_adapter(
            ChannelType::Telegram,
            Arc::new(StubChannelAdapter::new(ChannelType::Telegram)),
        );

        let channel = make_channel(ChannelType::Telegram, serde_json::json!({}));
        manager.connect(&channel).await.unwrap();
        assert!(manager.is_connected(&channel.id).await);
        assert_eq!(manager.connected_channels().await.len(), 1);

        manager
            .disconnect(&channel.id, ChannelType::Telegram)
            .await
            .unwrap();
        assert!(!manager.is_connected(&channel.id).await);
        assert!(manager.connected_channels().await.is_empty());
    }

    #[tokio::test]
    async fn manager_rejects_unregistered_type() {
        let manager = ChannelManager::new();
        let channel = make_channel(ChannelType::Slack, serde_json::json!({}));

        let result = manager.connect(&channel).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("no adapter registered")),
            _ => panic!("expected Channel error"),
        }
    }

    #[tokio::test]
    async fn manager_send_message() {
        let mut manager = ChannelManager::new();
        manager.register_adapter(
            ChannelType::Telegram,
            Arc::new(StubChannelAdapter::new(ChannelType::Telegram)),
        );

        let channel = make_channel(ChannelType::Telegram, serde_json::json!({}));
        let msg = make_outbound(channel.id);

        manager.connect(&channel).await.unwrap();
        manager
            .send_message(ChannelType::Telegram, &msg)
            .await
            .unwrap();
    }

    // -----------------------------------------------------------------------
    // TelegramAdapter (unit tests — see telegram module for more)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn telegram_requires_bot_token() {
        let adapter = TelegramAdapter::new();
        let channel = make_channel(ChannelType::Telegram, serde_json::json!({}));

        let result = adapter.connect(&channel).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("bot_token")),
            _ => panic!("expected Channel error"),
        }
    }

    #[tokio::test]
    async fn telegram_rejects_bad_token_format() {
        let adapter = TelegramAdapter::new();
        let channel = make_channel(
            ChannelType::Telegram,
            serde_json::json!({"bot_token": "nocolon"}),
        );

        let result = adapter.connect(&channel).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("invalid format")),
            _ => panic!("expected Channel error about format"),
        }
    }

    // -----------------------------------------------------------------------
    // LarkAdapter
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn lark_requires_app_credentials() {
        let adapter = LarkAdapter::new();
        let channel = make_channel(ChannelType::Lark, serde_json::json!({}));

        let result = adapter.connect(&channel).await;
        assert!(result.is_err());
        match result {
            Err(ClawxError::Channel(msg)) => assert!(msg.contains("app_id")),
            _ => panic!("expected Channel error"),
        }
    }

    #[tokio::test]
    async fn lark_disconnect_and_is_connected() {
        // LarkAdapter.connect() calls the real API, so we test
        // disconnect/is_connected without needing a server.
        let adapter = LarkAdapter::new();
        let cid = ChannelId::new();

        // Not connected initially.
        assert!(!adapter.is_connected(&cid).await);

        // Disconnect on unknown channel is a no-op (just removes from map).
        adapter.disconnect(&cid).await.unwrap();
        assert!(!adapter.is_connected(&cid).await);
    }

    #[tokio::test]
    async fn lark_send_requires_connection() {
        let adapter = LarkAdapter::new();
        let msg = OutboundMessage {
            channel_id: ChannelId::new(),
            content: "hello".to_string(),
            thread_id: None,
            reply_to: Some("oc_xxx".to_string()),
        };

        let err = adapter.send_message(&msg).await.unwrap_err();
        assert!(matches!(err, ClawxError::Channel(ref m) if m.contains("not connected")));
    }

    // -----------------------------------------------------------------------
    // MessageRouter
    // -----------------------------------------------------------------------

    #[test]
    fn route_message_with_bound_agent() {
        let manager = Arc::new(ChannelManager::new());
        let router = MessageRouter::new(manager);

        let agent_id = clawx_types::ids::AgentId::new();
        let mut channel = make_channel(ChannelType::Telegram, serde_json::json!({}));
        channel.agent_id = Some(agent_id);

        let result = router.route(&channel, "user-42", "Hello agent");
        assert!(result.is_some());
        let routed = result.unwrap();
        assert_eq!(routed.agent_id, agent_id);
        assert_eq!(routed.channel_id, channel.id);
        assert_eq!(routed.content, "Hello agent");
        assert_eq!(routed.sender_id, "user-42");
    }

    #[test]
    fn route_message_without_agent_returns_none() {
        let manager = Arc::new(ChannelManager::new());
        let router = MessageRouter::new(manager);

        let channel = make_channel(ChannelType::Telegram, serde_json::json!({}));
        assert!(channel.agent_id.is_none());

        let result = router.route(&channel, "user-42", "Hello?");
        assert!(result.is_none());
    }

    #[test]
    fn routed_message_has_correct_fields() {
        let manager = Arc::new(ChannelManager::new());
        let router = MessageRouter::new(manager);

        let agent_id = clawx_types::ids::AgentId::new();
        let mut channel = make_channel(ChannelType::Lark, serde_json::json!({}));
        channel.agent_id = Some(agent_id);

        let before = Utc::now();
        let routed = router.route(&channel, "sender-abc", "test content").unwrap();
        let after = Utc::now();

        assert_eq!(routed.channel_id, channel.id);
        assert_eq!(routed.agent_id, agent_id);
        assert_eq!(routed.content, "test content");
        assert_eq!(routed.sender_id, "sender-abc");
        assert!(routed.timestamp >= before);
        assert!(routed.timestamp <= after);
    }
}
