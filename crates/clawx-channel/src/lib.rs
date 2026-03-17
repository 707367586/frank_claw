//! IM channel adapters for ClawX (v0.2 stub).
//!
//! Will provide adapters for messaging platforms (Slack, Discord, Telegram, etc.)
//! to allow agents to interact through external communication channels.

/// Placeholder channel adapter trait.
#[async_trait::async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Send a message through this channel.
    async fn send_message(&self, content: &str) -> Result<(), Box<dyn std::error::Error>>;
}
