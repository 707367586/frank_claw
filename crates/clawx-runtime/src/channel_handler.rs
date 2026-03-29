//! Channel message handler: inbound message → route → conversation → agent loop → outbound reply.
//!
//! Bridges the IM channel system with the agent runtime.

use std::sync::Arc;

use clawx_channel::{ChannelManager, MessageRouter};
use clawx_eventbus::EventBusPort;
use clawx_types::channel::{InboundMessage, OutboundMessage};
use clawx_types::event::Event;
use clawx_types::ids::AgentId;
use tracing::{debug, info, warn};

use crate::{conversation_repo, Runtime};

/// Result of handling an inbound channel message.
#[derive(Debug)]
pub struct ChannelHandleResult {
    pub agent_id: AgentId,
    pub conversation_id: String,
    pub response_content: String,
    pub reply_sent: bool,
}

/// Handle an inbound message from a channel.
///
/// Flow:
/// 1. Route message to agent via MessageRouter
/// 2. Find or create a conversation for the sender
/// 3. Store the inbound message
/// 4. Run the agent loop
/// 5. Store the agent response
/// 6. Send the reply back through the channel
/// 7. Publish event to EventBus
pub async fn handle_inbound_message(
    runtime: &Runtime,
    channel_manager: &ChannelManager,
    router: &MessageRouter,
    event_bus: &Arc<dyn EventBusPort>,
    message: &InboundMessage,
) -> Result<ChannelHandleResult, String> {
    // 1. Look up channel and route to agent
    let channel = crate::channel_repo::get_channel(&runtime.db.main, &message.channel_id)
        .await
        .map_err(|e| format!("failed to get channel: {}", e))?
        .ok_or_else(|| format!("channel {} not found", message.channel_id))?;

    let routed = router.route(&channel, &message.sender_id, &message.content)
        .ok_or_else(|| format!("channel {} has no bound agent", message.channel_id))?;

    let agent_id = routed.agent_id;
    info!(%agent_id, channel_id = %message.channel_id, sender = %message.sender_id, "routing inbound message");

    // 2. Find or create conversation for this sender on this channel
    let agent_id_str = agent_id.to_string();
    let conversation_id = find_or_create_conversation(
        runtime,
        &agent_id_str,
        &message.sender_id,
        &message.channel_id.to_string(),
    ).await?;

    // 3. Store inbound message
    conversation_repo::add_message(
        &runtime.db.main,
        &conversation_id,
        "user",
        &message.content,
    )
    .await
    .map_err(|e| format!("failed to store inbound message: {}", e))?;

    // 4. Build conversation context and run agent loop
    let conversation = build_conversation(runtime, &agent_id, &conversation_id).await?;

    let agent_response = crate::agent_loop::run_turn(
        runtime,
        &agent_id,
        &conversation,
        &message.content,
    )
    .await
    .map_err(|e| format!("agent loop failed: {}", e))?;

    // 5. Store agent response
    conversation_repo::add_message(
        &runtime.db.main,
        &conversation_id,
        "assistant",
        &agent_response.content,
    )
    .await
    .map_err(|e| format!("failed to store agent response: {}", e))?;

    debug!(%agent_id, %conversation_id, "agent response stored");

    // 6. Send reply through channel
    let outbound = OutboundMessage {
        channel_id: message.channel_id,
        content: agent_response.content.clone(),
        thread_id: message.thread_id.clone(),
        reply_to: Some(message.sender_id.clone()),
    };

    let reply_sent = match channel_manager.send_message(channel.channel_type, &outbound).await {
        Ok(()) => {
            info!(%agent_id, channel_id = %message.channel_id, "reply sent");
            true
        }
        Err(e) => {
            warn!(%agent_id, channel_id = %message.channel_id, "failed to send reply: {}", e);
            false
        }
    };

    // 7. Publish event to EventBus
    let event = Event {
        id: clawx_types::ids::EventId::new(),
        timestamp: chrono::Utc::now(),
        source: "channel_handler".to_string(),
        kind: clawx_types::event::EventKind::ChannelMessageReceived,
        payload: Some(serde_json::json!({
            "channel_id": message.channel_id.to_string(),
            "agent_id": agent_id.to_string(),
            "sender_id": message.sender_id,
            "conversation_id": conversation_id,
            "reply_sent": reply_sent,
        })),
    };
    event_bus.publish(event).await;

    Ok(ChannelHandleResult {
        agent_id,
        conversation_id,
        response_content: agent_response.content,
        reply_sent,
    })
}

/// Find an existing conversation for this channel sender, or create a new one.
async fn find_or_create_conversation(
    runtime: &Runtime,
    agent_id: &str,
    sender_id: &str,
    channel_id: &str,
) -> Result<String, String> {
    // For now, create a new conversation per sender+channel combination.
    // A more sophisticated approach would use thread_id or lookup by metadata.
    // We use a deterministic title to enable future lookup.
    let title = format!("channel:{}:sender:{}", channel_id, sender_id);

    // Try to find existing conversation with this title (indexed lookup)
    if let Some(existing_id) = conversation_repo::find_conversation_by_title(
        &runtime.db.main, agent_id, &title,
    )
    .await
    .map_err(|e| format!("failed to find conversation: {}", e))?
    {
        return Ok(existing_id);
    }

    // Create new conversation
    let conv_id = conversation_repo::create_conversation(
        &runtime.db.main,
        agent_id,
        Some(&title),
    )
    .await
    .map_err(|e| format!("failed to create conversation: {}", e))?;

    info!(agent_id, %conv_id, %title, "created conversation for channel sender");
    Ok(conv_id)
}

/// Build a Conversation struct from DB for agent_loop.
async fn build_conversation(
    runtime: &Runtime,
    agent_id: &clawx_types::ids::AgentId,
    conversation_id: &str,
) -> Result<clawx_types::agent::Conversation, String> {
    use clawx_types::agent::ConversationMessage;
    use clawx_types::ids::{ConversationId, MessageId};

    let messages_json = conversation_repo::list_messages(&runtime.db.main, conversation_id)
        .await
        .map_err(|e| format!("failed to list messages: {}", e))?;

    let conv_id: ConversationId = conversation_id
        .parse()
        .unwrap_or_else(|_| ConversationId::new());

    let messages: Vec<ConversationMessage> = messages_json
        .iter()
        .filter_map(|m| {
            let role_str = m.get("role")?.as_str()?;
            let content = m.get("content")?.as_str()?.to_string();
            let role = match role_str {
                "user" => clawx_types::llm::MessageRole::User,
                "assistant" => clawx_types::llm::MessageRole::Assistant,
                "system" => clawx_types::llm::MessageRole::System,
                _ => return None,
            };
            let id_str = m.get("id")?.as_str()?;
            let msg_id: MessageId = id_str.parse().unwrap_or_else(|_| MessageId::new());
            let created_at = m.get("created_at")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(chrono::Utc::now);
            Some(ConversationMessage {
                id: msg_id,
                conversation_id: conv_id,
                role,
                content,
                metadata: None,
                created_at,
            })
        })
        .collect();

    Ok(clawx_types::agent::Conversation {
        id: conv_id,
        agent_id: *agent_id,
        title: None,
        status: clawx_types::agent::ConversationStatus::Active,
        messages,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::agent::AgentConfig;
    use clawx_types::ids::{AgentId, ChannelId};

    async fn make_runtime() -> Runtime {
        Runtime::new(
            crate::db::Database::in_memory().await.unwrap(),
            Arc::new(clawx_llm::StubLlmProvider),
            Arc::new(clawx_memory::StubMemoryService),
            Arc::new(clawx_memory::StubWorkingMemoryManager),
            Arc::new(clawx_memory::StubMemoryExtractor),
            Arc::new(clawx_security::PermissiveSecurityGuard),
            Arc::new(clawx_vault::StubVaultService),
            Arc::new(clawx_kb::StubKnowledgeService),
            Arc::new(clawx_config::ConfigLoader::with_defaults()),
        )
    }

    fn make_agent_config(agent_id: AgentId) -> AgentConfig {
        AgentConfig {
            id: agent_id,
            name: "Test Agent".into(),
            role: "assistant".into(),
            model_id: clawx_types::ids::ProviderId::new(),
            system_prompt: None,
            icon: None,
            status: clawx_types::agent::AgentStatus::Active,
            capabilities: vec![],
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            last_active_at: None,
        }
    }

    #[tokio::test]
    async fn find_or_create_conversation_creates_new() {
        let rt = make_runtime().await;
        let agent_id = AgentId::new();
        let channel_id = ChannelId::new();

        crate::agent_repo::create_agent(&rt.db.main, &make_agent_config(agent_id)).await.unwrap();

        let agent_id_str = agent_id.to_string();
        let conv_id = find_or_create_conversation(
            &rt,
            &agent_id_str,
            "user123",
            &channel_id.to_string(),
        ).await.unwrap();

        assert!(!conv_id.is_empty());

        // Second call should return the same conversation
        let conv_id2 = find_or_create_conversation(
            &rt,
            &agent_id_str,
            "user123",
            &channel_id.to_string(),
        ).await.unwrap();

        assert_eq!(conv_id, conv_id2);
    }

    #[tokio::test]
    async fn different_senders_get_different_conversations() {
        let rt = make_runtime().await;
        let agent_id = AgentId::new();
        let channel_id = ChannelId::new();

        crate::agent_repo::create_agent(&rt.db.main, &make_agent_config(agent_id)).await.unwrap();

        let agent_id_str = agent_id.to_string();
        let conv1 = find_or_create_conversation(
            &rt, &agent_id_str, "alice", &channel_id.to_string(),
        ).await.unwrap();

        let conv2 = find_or_create_conversation(
            &rt, &agent_id_str, "bob", &channel_id.to_string(),
        ).await.unwrap();

        assert_ne!(conv1, conv2);
    }

    #[tokio::test]
    async fn handle_inbound_routes_and_responds() {
        let rt = make_runtime().await;
        let agent_id = AgentId::new();
        let channel_id = ChannelId::new();

        crate::agent_repo::create_agent(&rt.db.main, &make_agent_config(agent_id)).await.unwrap();

        // Set up channel bound to agent
        let channel = clawx_types::channel::Channel {
            id: channel_id,
            channel_type: clawx_types::channel::ChannelType::Telegram,
            name: "Test Channel".into(),
            config: serde_json::json!({"bot_token": "fake"}),
            agent_id: Some(agent_id),
            status: clawx_types::channel::ChannelStatus::Connected,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        };
        crate::channel_repo::create_channel(&rt.db.main, &channel)
            .await
            .unwrap();

        // Set up channel manager with stub adapter
        let mut manager = ChannelManager::new();
        manager.register_adapter(
            clawx_types::channel::ChannelType::Telegram,
            Arc::new(clawx_channel::StubChannelAdapter::new(clawx_types::channel::ChannelType::Telegram)),
        );
        let manager_arc = Arc::new(manager);
        let router = MessageRouter::new(manager_arc.clone());

        let event_bus: Arc<dyn EventBusPort> = Arc::new(clawx_eventbus::NoopEventBus);

        let inbound = InboundMessage {
            channel_id,
            sender_id: "user456".into(),
            content: "Hello from Telegram!".into(),
            thread_id: None,
            is_direct_message: true,
            received_at: chrono::Utc::now(),
        };

        let result = handle_inbound_message(&rt, &manager_arc, &router, &event_bus, &inbound).await;
        assert!(result.is_ok(), "expected ok, got: {:?}", result.err());

        let result = result.unwrap();
        assert_eq!(result.agent_id, agent_id);
        assert!(!result.conversation_id.is_empty());
        // StubLlmProvider returns "[stub] ..."
        assert!(result.response_content.contains("[stub]"));
    }
}
