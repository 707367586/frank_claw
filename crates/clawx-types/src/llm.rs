use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::ProviderId;

/// Role of a message participant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A single message in an LLM conversation.
///
/// `content` remains the plain-text channel for back-compat with older code paths.
/// `blocks` is additive: when non-empty it carries structured content
/// (`tool_use`, `tool_result`) that providers serialize
/// into their native block formats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<ContentBlock>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Structured content for a `Message`.
///
/// Mirrors Anthropic's content-block schema. Providers that use a flat
/// `tool_calls` field (OpenAI) translate to/from this representation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// Phase 1: `content` is a plain string. Anthropic also allows nested
    /// content blocks here — revisit when we need image/tool-nested results.
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

/// Definition of a tool the model may invoke.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// A request sent to an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stream: bool,
}

/// Reason the model stopped generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    ToolUse,
    StopSequence,
}

/// A tool call requested by the model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

/// Token usage statistics for a completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Full response from an LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmResponse {
    pub content: String,
    pub stop_reason: StopReason,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<ProviderMetadata>,
}

/// A single chunk from a streaming LLM response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStreamChunk {
    pub delta: String,
    pub stop_reason: Option<StopReason>,
    pub usage: Option<TokenUsage>,
}

/// Opaque metadata returned by a specific LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetadata {
    pub provider: String,
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extra: Option<serde_json::Value>,
}

/// Type of LLM provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderType {
    Openai,
    Anthropic,
    Zhipu,
    Ollama,
    Custom,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Openai => write!(f, "openai"),
            Self::Anthropic => write!(f, "anthropic"),
            Self::Zhipu => write!(f, "zhipu"),
            Self::Ollama => write!(f, "ollama"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "openai" => Ok(Self::Openai),
            "anthropic" => Ok(Self::Anthropic),
            "zhipu" => Ok(Self::Zhipu),
            "ollama" => Ok(Self::Ollama),
            "custom" => Ok(Self::Custom),
            other => Err(format!("unknown provider type: {}", other)),
        }
    }
}

/// Configuration for an LLM provider.
/// Aligned with `llm_providers` table in data-model.md §2.4.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmProviderConfig {
    pub id: ProviderId,
    pub name: String,
    pub provider_type: ProviderType,
    pub base_url: String,
    pub model_name: String,
    /// JSON: temperature, max_tokens, etc.
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub is_default: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod content_block_tests {
    use super::*;

    #[test]
    fn message_without_blocks_serializes_back_compat() {
        let m = Message {
            role: MessageRole::User,
            content: "hi".into(),
            blocks: vec![],
            tool_call_id: None,
        };
        let s = serde_json::to_string(&m).unwrap();
        // Back-compat: no `blocks` field in wire when empty.
        assert!(!s.contains("blocks"));
        let back: Message = serde_json::from_str(&s).unwrap();
        assert_eq!(back.content, "hi");
        assert!(back.blocks.is_empty());
    }

    #[test]
    fn tool_use_block_round_trips() {
        let b = ContentBlock::ToolUse {
            id: "call_1".into(),
            name: "fs_mkdir".into(),
            input: serde_json::json!({"path": "/tmp/foo"}),
        };
        let s = serde_json::to_string(&b).unwrap();
        assert!(s.contains(r#""type":"tool_use""#));
        let back: ContentBlock = serde_json::from_str(&s).unwrap();
        match back {
            ContentBlock::ToolUse { id, name, .. } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "fs_mkdir");
            }
            _ => panic!("expected ToolUse"),
        }
    }

    #[test]
    fn tool_result_block_round_trips_with_is_error() {
        let b = ContentBlock::ToolResult {
            tool_use_id: "call_1".into(),
            content: "ok".into(),
            is_error: true,
        };
        let s = serde_json::to_string(&b).unwrap();
        let back: ContentBlock = serde_json::from_str(&s).unwrap();
        match back {
            ContentBlock::ToolResult { is_error, .. } => assert!(is_error),
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn tool_result_is_error_false_is_omitted_on_wire() {
        let b = ContentBlock::ToolResult {
            tool_use_id: "call_1".into(),
            content: "ok".into(),
            is_error: false,
        };
        let s = serde_json::to_string(&b).unwrap();
        assert!(
            !s.contains("is_error"),
            "is_error=false must be omitted, got: {s}"
        );
    }

    #[test]
    fn tool_result_missing_is_error_defaults_false() {
        let raw = r#"{"type":"tool_result","tool_use_id":"x","content":"ok"}"#;
        let back: ContentBlock = serde_json::from_str(raw).unwrap();
        match back {
            ContentBlock::ToolResult { is_error, .. } => assert!(!is_error),
            _ => panic!("expected ToolResult"),
        }
    }

    #[test]
    fn text_block_round_trips() {
        let b = ContentBlock::Text {
            text: "hello".into(),
        };
        let s = serde_json::to_string(&b).unwrap();
        assert!(s.contains(r#""type":"text""#));
        let back: ContentBlock = serde_json::from_str(&s).unwrap();
        match back {
            ContentBlock::Text { text } => assert_eq!(text, "hello"),
            _ => panic!("expected Text"),
        }
    }
}
