use clawx_types::*;
use clawx_types::memory::*;
use clawx_types::llm::*;
use clawx_types::security::*;
use clawx_types::agent::*;
use clawx_types::vault::*;
use clawx_types::knowledge::*;
use clawx_types::config::*;
use clawx_types::event::*;

// ---------------------------------------------------------------------------
// ID types
// ---------------------------------------------------------------------------

#[test]
fn id_new_returns_unique() {
    let a = AgentId::new();
    let b = AgentId::new();
    assert_ne!(a, b);
}

#[test]
fn id_default_returns_unique() {
    let a = MemoryId::default();
    let b = MemoryId::default();
    assert_ne!(a, b);
}

#[test]
fn id_display() {
    let id = AgentId::new();
    let s = id.to_string();
    assert!(!s.is_empty());
    assert!(s.contains('-')); // UUID format
}

#[test]
fn id_serde_roundtrip() {
    let id = AgentId::new();
    let json = serde_json::to_string(&id).unwrap();
    let parsed: AgentId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn id_uuid_conversion() {
    let id = ConversationId::new();
    let uuid: uuid::Uuid = id.into();
    let back: ConversationId = uuid.into();
    assert_eq!(id, back);
}

#[test]
fn all_id_types_serde() {
    // Ensure all ID types serialize/deserialize correctly
    let ids: Vec<String> = vec![
        serde_json::to_string(&AgentId::new()).unwrap(),
        serde_json::to_string(&TaskId::new()).unwrap(),
        serde_json::to_string(&MemoryId::new()).unwrap(),
        serde_json::to_string(&SkillId::new()).unwrap(),
        serde_json::to_string(&ChannelId::new()).unwrap(),
        serde_json::to_string(&EventId::new()).unwrap(),
        serde_json::to_string(&SnapshotId::new()).unwrap(),
        serde_json::to_string(&ConversationId::new()).unwrap(),
        serde_json::to_string(&MessageId::new()).unwrap(),
        serde_json::to_string(&ChunkId::new()).unwrap(),
        serde_json::to_string(&DocumentId::new()).unwrap(),
        serde_json::to_string(&KnowledgeSourceId::new()).unwrap(),
        serde_json::to_string(&ProviderId::new()).unwrap(),
        serde_json::to_string(&SessionId::new()).unwrap(),
        serde_json::to_string(&AuditEntryId::new()).unwrap(),
    ];
    for json in ids {
        assert!(json.starts_with('"'));
        assert!(json.ends_with('"'));
    }
}

// ---------------------------------------------------------------------------
// Enum serde
// ---------------------------------------------------------------------------

#[test]
fn memory_scope_serde() {
    assert_eq!(serde_json::to_string(&MemoryScope::Agent).unwrap(), "\"agent\"");
    assert_eq!(serde_json::to_string(&MemoryScope::User).unwrap(), "\"user\"");
    let parsed: MemoryScope = serde_json::from_str("\"agent\"").unwrap();
    assert_eq!(parsed, MemoryScope::Agent);
}

#[test]
fn memory_kind_serde() {
    let kinds = vec![
        (MemoryKind::Fact, "\"fact\""),
        (MemoryKind::Preference, "\"preference\""),
        (MemoryKind::Event, "\"event\""),
        (MemoryKind::Skill, "\"skill\""),
        (MemoryKind::Contact, "\"contact\""),
        (MemoryKind::Terminology, "\"terminology\""),
    ];
    for (kind, expected) in kinds {
        assert_eq!(serde_json::to_string(&kind).unwrap(), expected);
    }
}

#[test]
fn source_type_serde() {
    assert_eq!(serde_json::to_string(&SourceType::Implicit).unwrap(), "\"implicit\"");
    assert_eq!(serde_json::to_string(&SourceType::Explicit).unwrap(), "\"explicit\"");
    assert_eq!(serde_json::to_string(&SourceType::Consolidation).unwrap(), "\"consolidation\"");
}

#[test]
fn agent_status_serde() {
    assert_eq!(serde_json::to_string(&AgentStatus::Idle).unwrap(), "\"idle\"");
    assert_eq!(serde_json::to_string(&AgentStatus::Active).unwrap(), "\"active\"");
    assert_eq!(serde_json::to_string(&AgentStatus::Error).unwrap(), "\"error\"");
    assert_eq!(serde_json::to_string(&AgentStatus::Offline).unwrap(), "\"offline\"");
}

#[test]
fn message_role_serde() {
    assert_eq!(serde_json::to_string(&MessageRole::System).unwrap(), "\"system\"");
    assert_eq!(serde_json::to_string(&MessageRole::User).unwrap(), "\"user\"");
    assert_eq!(serde_json::to_string(&MessageRole::Assistant).unwrap(), "\"assistant\"");
    assert_eq!(serde_json::to_string(&MessageRole::Tool).unwrap(), "\"tool\"");
}

#[test]
fn stop_reason_serde() {
    assert_eq!(serde_json::to_string(&StopReason::EndTurn).unwrap(), "\"end_turn\"");
    assert_eq!(serde_json::to_string(&StopReason::ToolUse).unwrap(), "\"tool_use\"");
}

#[test]
fn provider_type_serde() {
    assert_eq!(serde_json::to_string(&ProviderType::Openai).unwrap(), "\"openai\"");
    assert_eq!(serde_json::to_string(&ProviderType::Anthropic).unwrap(), "\"anthropic\"");
    assert_eq!(serde_json::to_string(&ProviderType::Ollama).unwrap(), "\"ollama\"");
    assert_eq!(serde_json::to_string(&ProviderType::Custom).unwrap(), "\"custom\"");
}

#[test]
fn security_decision_serde() {
    let allow = SecurityDecision::Allow;
    assert_eq!(serde_json::to_string(&allow).unwrap(), "\"allow\"");

    let deny = SecurityDecision::Deny { reason: "forbidden".into() };
    let json = serde_json::to_string(&deny).unwrap();
    let parsed: SecurityDecision = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, deny);
}

#[test]
fn capability_serde() {
    assert_eq!(serde_json::to_string(&Capability::FsRead).unwrap(), "\"fs_read\"");
    assert_eq!(serde_json::to_string(&Capability::NetHttp).unwrap(), "\"net_http\"");
}

#[test]
fn change_type_serde() {
    assert_eq!(serde_json::to_string(&ChangeType::Added).unwrap(), "\"added\"");
    assert_eq!(serde_json::to_string(&ChangeType::Modified).unwrap(), "\"modified\"");
    assert_eq!(serde_json::to_string(&ChangeType::Deleted).unwrap(), "\"deleted\"");
    assert_eq!(serde_json::to_string(&ChangeType::Renamed).unwrap(), "\"renamed\"");
}

#[test]
fn event_kind_serde() {
    assert_eq!(serde_json::to_string(&EventKind::AgentStarted).unwrap(), "\"agent_started\"");
    assert_eq!(serde_json::to_string(&EventKind::MemoryStored).unwrap(), "\"memory_stored\"");
}

#[test]
fn conversation_status_serde() {
    assert_eq!(serde_json::to_string(&ConversationStatus::Active).unwrap(), "\"active\"");
    assert_eq!(serde_json::to_string(&ConversationStatus::Archived).unwrap(), "\"archived\"");
}

#[test]
fn knowledge_source_status_serde() {
    assert_eq!(serde_json::to_string(&KnowledgeSourceStatus::Active).unwrap(), "\"active\"");
    assert_eq!(serde_json::to_string(&KnowledgeSourceStatus::Error).unwrap(), "\"error\"");
}

#[test]
fn document_status_serde() {
    assert_eq!(serde_json::to_string(&DocumentStatus::Pending).unwrap(), "\"pending\"");
    assert_eq!(serde_json::to_string(&DocumentStatus::Indexed).unwrap(), "\"indexed\"");
}

// ---------------------------------------------------------------------------
// Struct defaults
// ---------------------------------------------------------------------------

#[test]
fn source_type_default() {
    assert_eq!(SourceType::default(), SourceType::Implicit);
}

#[test]
fn token_usage_default() {
    let usage = TokenUsage::default();
    assert_eq!(usage.prompt_tokens, 0);
    assert_eq!(usage.completion_tokens, 0);
    assert_eq!(usage.total_tokens, 0);
}

#[test]
fn memory_filter_default() {
    let filter = MemoryFilter::default();
    assert!(filter.scope.is_none());
    assert!(filter.agent_id.is_none());
    assert!(!filter.include_archived);
}

#[test]
fn pagination_default() {
    let p = Pagination::default();
    assert_eq!(p.page, 1);
    assert_eq!(p.per_page, 20);
}

#[test]
fn paged_result_total_pages() {
    let result: PagedResult<()> = PagedResult {
        items: vec![],
        total: 55,
        page: 1,
        per_page: 20,
    };
    assert_eq!(result.total_pages(), 3);
}

#[test]
fn paged_result_total_pages_exact() {
    let result: PagedResult<()> = PagedResult {
        items: vec![],
        total: 40,
        page: 1,
        per_page: 20,
    };
    assert_eq!(result.total_pages(), 2);
}

// ---------------------------------------------------------------------------
// Config defaults
// ---------------------------------------------------------------------------

#[test]
fn clawx_config_defaults() {
    let config: ClawxConfig = toml::from_str("").unwrap();
    assert_eq!(config.general.language, "en");
    assert_eq!(config.general.max_active_agents, 3);
    assert_eq!(config.storage.data_dir, "~/.clawx");
    assert!(config.api.dev_port.is_none());
}

// ---------------------------------------------------------------------------
// Error display
// ---------------------------------------------------------------------------

#[test]
fn error_display() {
    let e = ClawxError::NotFound {
        entity: "Agent".into(),
        id: "123".into(),
    };
    assert_eq!(format!("{e}"), "not found: Agent 123");

    let e = ClawxError::SecurityDenied {
        reason: "test".into(),
    };
    assert!(format!("{e}").contains("test"));
}
