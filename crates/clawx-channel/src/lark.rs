//! Lark/Feishu channel adapter — real API implementation.
//!
//! Authenticates via tenant_access_token and sends messages through the
//! Lark Open API.  WebSocket-based receive is not yet implemented.

use std::collections::HashMap;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::RwLock;

use clawx_types::channel::{Channel, OutboundMessage};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::ChannelId;

use crate::ChannelAdapter;

// ---------------------------------------------------------------------------
// Lark API response types (private)
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize)]
struct TokenResponse {
    code: i32,
    msg: String,
    tenant_access_token: Option<String>,
    expire: Option<i64>,
}

#[derive(serde::Deserialize)]
struct SendMessageResponse {
    code: i32,
    msg: String,
}

// ---------------------------------------------------------------------------
// Connection state
// ---------------------------------------------------------------------------

struct LarkConnection {
    app_id: String,
    app_secret: String,
    tenant_access_token: String,
    token_expires_at: DateTime<Utc>,
    default_chat_id: Option<String>,
}

// ---------------------------------------------------------------------------
// LarkAdapter
// ---------------------------------------------------------------------------

/// Lark/Feishu adapter that calls the real Open API.
pub struct LarkAdapter {
    client: reqwest::Client,
    connections: RwLock<HashMap<ChannelId, LarkConnection>>,
    /// Base URL for the Lark API — overridable for testing.
    base_url: String,
}

impl LarkAdapter {
    /// Create a new adapter pointing at the production Feishu API.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            connections: RwLock::new(HashMap::new()),
            base_url: "https://open.feishu.cn".to_string(),
        }
    }

    /// Create an adapter with a custom base URL (for tests / mocks).
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn with_base_url(base_url: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            connections: RwLock::new(HashMap::new()),
            base_url: base_url.to_string(),
        }
    }

    /// Fetch a new tenant_access_token from the Lark API.
    async fn fetch_token(&self, app_id: &str, app_secret: &str) -> Result<(String, DateTime<Utc>)> {
        let url = format!(
            "{}/open-apis/auth/v3/tenant_access_token/internal",
            self.base_url
        );

        let body = serde_json::json!({
            "app_id": app_id,
            "app_secret": app_secret,
        });

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::Channel(format!("lark token request failed: {e}")))?;

        let status = resp.status();
        let token_resp: TokenResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::Channel(format!("lark token response parse error: {e}")))?;

        if !status.is_success() || token_resp.code != 0 {
            return Err(ClawxError::Channel(format!(
                "lark token error: code={}, msg={}",
                token_resp.code, token_resp.msg
            )));
        }

        let token = token_resp
            .tenant_access_token
            .ok_or_else(|| ClawxError::Channel("lark token response missing token".to_string()))?;

        let expire_secs = token_resp.expire.unwrap_or(7200);
        // Subtract 5 minutes as a safety margin for refresh.
        let expires_at = Utc::now() + chrono::Duration::seconds(expire_secs - 300);

        Ok((token, expires_at))
    }

    /// Refresh the token for a connection if it has expired or is about to.
    async fn ensure_fresh_token(&self, channel_id: &ChannelId) -> Result<()> {
        // First check under a read lock whether refresh is needed.
        let needs_refresh = {
            let conns = self.connections.read().await;
            match conns.get(channel_id) {
                Some(conn) => Utc::now() >= conn.token_expires_at,
                None => {
                    return Err(ClawxError::Channel(format!(
                        "lark channel {channel_id} not connected"
                    )));
                }
            }
        };

        if needs_refresh {
            // Read app_id/app_secret under read lock, then drop it before the
            // network call so we don't hold the lock across await.
            let (app_id, app_secret) = {
                let conns = self.connections.read().await;
                let conn = conns.get(channel_id).ok_or_else(|| {
                    ClawxError::Channel(format!("lark channel {channel_id} not connected"))
                })?;
                (conn.app_id.clone(), conn.app_secret.clone())
            };

            let (new_token, new_expires) = self.fetch_token(&app_id, &app_secret).await?;

            let mut conns = self.connections.write().await;
            if let Some(conn) = conns.get_mut(channel_id) {
                conn.tenant_access_token = new_token;
                conn.token_expires_at = new_expires;
            }
        }

        Ok(())
    }
}

impl Default for LarkAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ChannelAdapter for LarkAdapter {
    async fn connect(&self, channel: &Channel) -> Result<()> {
        let app_id = channel
            .config
            .get("app_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ClawxError::Channel(
                    "lark channel requires 'app_id' and 'app_secret' in config".to_string(),
                )
            })?
            .to_string();

        let app_secret = channel
            .config
            .get("app_secret")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ClawxError::Channel(
                    "lark channel requires 'app_id' and 'app_secret' in config".to_string(),
                )
            })?
            .to_string();

        let default_chat_id = channel
            .config
            .get("chat_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Fetch initial token
        let (token, expires_at) = self.fetch_token(&app_id, &app_secret).await?;

        tracing::info!(
            channel_id = %channel.id,
            "lark adapter: connected"
        );

        let mut conns = self.connections.write().await;
        conns.insert(
            channel.id,
            LarkConnection {
                app_id,
                app_secret,
                tenant_access_token: token,
                token_expires_at: expires_at,
                default_chat_id,
            },
        );

        Ok(())
    }

    async fn disconnect(&self, channel_id: &ChannelId) -> Result<()> {
        let mut conns = self.connections.write().await;
        conns.remove(channel_id);
        tracing::info!(channel_id = %channel_id, "lark adapter: disconnected");
        Ok(())
    }

    async fn send_message(&self, msg: &OutboundMessage) -> Result<()> {
        self.ensure_fresh_token(&msg.channel_id).await?;

        let (token, chat_id) = {
            let conns = self.connections.read().await;
            let conn = conns.get(&msg.channel_id).ok_or_else(|| {
                ClawxError::Channel(format!(
                    "lark channel {} not connected",
                    msg.channel_id
                ))
            })?;

            // Use reply_to as chat_id override, otherwise fall back to default.
            let chat_id = msg
                .reply_to
                .clone()
                .or_else(|| conn.default_chat_id.clone())
                .ok_or_else(|| {
                    ClawxError::Channel(
                        "lark send_message: no chat_id (provide reply_to or configure default chat_id)".to_string(),
                    )
                })?;

            (conn.tenant_access_token.clone(), chat_id)
        };

        let url = format!(
            "{}/open-apis/im/v1/messages?receive_id_type=chat_id",
            self.base_url
        );

        let content = serde_json::json!({ "text": msg.content }).to_string();
        let body = serde_json::json!({
            "receive_id": chat_id,
            "msg_type": "text",
            "content": content,
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {token}"))
            .json(&body)
            .send()
            .await
            .map_err(|e| ClawxError::Channel(format!("lark send request failed: {e}")))?;

        let status = resp.status();
        let send_resp: SendMessageResponse = resp
            .json()
            .await
            .map_err(|e| ClawxError::Channel(format!("lark send response parse error: {e}")))?;

        if !status.is_success() || send_resp.code != 0 {
            return Err(ClawxError::Channel(format!(
                "lark send error: code={}, msg={}",
                send_resp.code, send_resp.msg
            )));
        }

        tracing::debug!(
            channel_id = %msg.channel_id,
            chat_id = %chat_id,
            "lark adapter: message sent"
        );

        Ok(())
    }

    async fn is_connected(&self, channel_id: &ChannelId) -> bool {
        let conns = self.connections.read().await;
        conns.contains_key(channel_id)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::channel::{ChannelStatus, ChannelType};

    fn make_lark_channel(config: serde_json::Value) -> Channel {
        let now = Utc::now();
        Channel {
            id: ChannelId::new(),
            channel_type: ChannelType::Lark,
            name: "test-lark".to_string(),
            config,
            agent_id: None,
            status: ChannelStatus::Disconnected,
            created_at: now,
            updated_at: now,
        }
    }

    #[tokio::test]
    async fn lark_connect_validates_credentials() {
        // Start a mock server that will never be reached — config validation
        // should fail before any HTTP call.
        let adapter = LarkAdapter::new();

        // Missing both
        let ch = make_lark_channel(serde_json::json!({}));
        let err = adapter.connect(&ch).await.unwrap_err();
        assert!(matches!(err, ClawxError::Channel(ref m) if m.contains("app_id")));

        // Missing app_secret
        let ch = make_lark_channel(serde_json::json!({"app_id": "cli_xxx"}));
        let err = adapter.connect(&ch).await.unwrap_err();
        assert!(matches!(err, ClawxError::Channel(ref m) if m.contains("app_secret")));
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

    #[tokio::test]
    async fn lark_disconnect_clears_state() {
        // We can't connect without a real/mock server, so manually insert a
        // connection to test disconnect logic.
        let adapter = LarkAdapter::new();
        let cid = ChannelId::new();

        {
            let mut conns = adapter.connections.write().await;
            conns.insert(
                cid,
                LarkConnection {
                    app_id: "cli_test".to_string(),
                    app_secret: "secret".to_string(),
                    tenant_access_token: "tok".to_string(),
                    token_expires_at: Utc::now() + chrono::Duration::hours(1),
                    default_chat_id: None,
                },
            );
        }

        assert!(adapter.is_connected(&cid).await);
        adapter.disconnect(&cid).await.unwrap();
        assert!(!adapter.is_connected(&cid).await);
    }

    /// Integration test — requires real Lark app credentials.
    /// Run with: LARK_APP_ID=xxx LARK_APP_SECRET=xxx LARK_CHAT_ID=oc_xxx cargo test -p clawx-channel lark_real_api -- --ignored
    #[tokio::test]
    #[ignore]
    async fn lark_real_api() {
        let app_id = std::env::var("LARK_APP_ID").expect("LARK_APP_ID");
        let app_secret = std::env::var("LARK_APP_SECRET").expect("LARK_APP_SECRET");
        let chat_id = std::env::var("LARK_CHAT_ID").expect("LARK_CHAT_ID");

        let adapter = LarkAdapter::new();
        let channel = make_lark_channel(serde_json::json!({
            "app_id": app_id,
            "app_secret": app_secret,
            "chat_id": chat_id,
        }));

        adapter.connect(&channel).await.unwrap();
        assert!(adapter.is_connected(&channel.id).await);

        let msg = OutboundMessage {
            channel_id: channel.id,
            content: "Hello from ClawX integration test!".to_string(),
            thread_id: None,
            reply_to: None,
        };
        adapter.send_message(&msg).await.unwrap();

        adapter.disconnect(&channel.id).await.unwrap();
        assert!(!adapter.is_connected(&channel.id).await);
    }
}
