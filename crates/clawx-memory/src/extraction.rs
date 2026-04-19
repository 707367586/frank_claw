//! LLM-assisted memory extraction from conversation messages.
//!
//! Analyzes conversation turns to identify facts, preferences, events,
//! and other memorable information worth persisting.

use async_trait::async_trait;
use std::sync::Arc;

use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use clawx_types::llm::*;
use clawx_types::memory::*;
use clawx_types::traits::{LlmProvider, MemoryExtractor};

/// LLM-based memory extractor that uses a prompt to identify extractable memories.
pub struct LlmMemoryExtractor {
    llm: Arc<dyn LlmProvider>,
    model: String,
}

impl LlmMemoryExtractor {
    /// Create a new extractor with the given LLM provider and model name.
    pub fn new(llm: Arc<dyn LlmProvider>, model: String) -> Self {
        Self { llm, model }
    }

    /// Build the extraction prompt for the given messages.
    fn build_prompt(messages: &[Message]) -> String {
        let mut conversation = String::new();
        for msg in messages {
            let role = match msg.role {
                MessageRole::User => "User",
                MessageRole::Assistant => "Assistant",
                MessageRole::System => "System",
                MessageRole::Tool => "Tool",
            };
            conversation.push_str(&format!("{}: {}\n", role, msg.content));
        }

        format!(
            r#"Analyze the following conversation and extract any memorable information.

For each memory, output a JSON object on its own line with these fields:
- "scope": "agent" or "user"
- "kind": one of "fact", "preference", "event", "skill", "contact", "terminology"
- "summary": a concise one-line summary
- "content": the detailed content as a string
- "importance": a number from 1-10

Only extract genuinely useful information. Do not extract trivial greetings or filler.
If there is nothing worth remembering, output nothing.

Conversation:
{}

Extracted memories (one JSON per line):"#,
            conversation
        )
    }

    /// Parse the LLM response into memory candidates.
    fn parse_candidates(
        response: &str,
        agent_id: &AgentId,
    ) -> Vec<MemoryCandidate> {
        let mut candidates = Vec::new();

        for line in response.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || !trimmed.starts_with('{') {
                continue;
            }

            if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let scope = match value["scope"].as_str().unwrap_or("agent") {
                    "user" => MemoryScope::User,
                    _ => MemoryScope::Agent,
                };

                let kind = match value["kind"].as_str().unwrap_or("fact") {
                    "preference" => MemoryKind::Preference,
                    "event" => MemoryKind::Event,
                    "skill" => MemoryKind::Skill,
                    "contact" => MemoryKind::Contact,
                    "terminology" => MemoryKind::Terminology,
                    _ => MemoryKind::Fact,
                };

                let summary = value["summary"]
                    .as_str()
                    .unwrap_or("extracted memory")
                    .to_string();

                let content = value["content"]
                    .as_str()
                    .map(|s| serde_json::json!({"text": s}))
                    .unwrap_or_else(|| serde_json::json!({"text": summary.clone()}));

                let importance = value["importance"].as_f64().unwrap_or(5.0).clamp(1.0, 10.0);

                candidates.push(MemoryCandidate {
                    scope,
                    kind,
                    summary,
                    content,
                    importance,
                    source_type: SourceType::Implicit,
                });
            }
        }

        // Tag with agent context (not stored in candidate but used during storage)
        let _ = agent_id;
        candidates
    }
}

#[async_trait]
impl MemoryExtractor for LlmMemoryExtractor {
    async fn extract(
        &self,
        agent_id: &AgentId,
        messages: &[Message],
    ) -> Result<Vec<MemoryCandidate>> {
        if messages.is_empty() {
            return Ok(vec![]);
        }

        let prompt = Self::build_prompt(messages);

        let request = CompletionRequest {
            model: self.model.clone(),
            messages: vec![Message {
                role: MessageRole::User,
                content: prompt,
                blocks: vec![],
                tool_call_id: None,
            }],
            tools: None,
            temperature: Some(0.3),
            max_tokens: Some(2048),
            stream: false,
        };

        let response = self.llm.complete(request).await?;
        let candidates = Self::parse_candidates(&response.content, agent_id);

        Ok(candidates)
    }
}

/// A stub extractor that always returns no candidates (for testing).
#[derive(Debug, Clone)]
pub struct StubMemoryExtractor;

#[async_trait]
impl MemoryExtractor for StubMemoryExtractor {
    async fn extract(
        &self,
        _agent_id: &AgentId,
        _messages: &[Message],
    ) -> Result<Vec<MemoryCandidate>> {
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------
    // parse_candidates tests
    // -------------------------------------------------------------------

    #[test]
    fn parse_empty_response() {
        let candidates = LlmMemoryExtractor::parse_candidates("", &AgentId::new());
        assert!(candidates.is_empty());
    }

    #[test]
    fn parse_single_candidate() {
        let response = r#"{"scope": "user", "kind": "preference", "summary": "Prefers dark mode", "content": "The user mentioned they prefer dark mode in all apps", "importance": 7}"#;
        let candidates = LlmMemoryExtractor::parse_candidates(response, &AgentId::new());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scope, MemoryScope::User);
        assert_eq!(candidates[0].kind, MemoryKind::Preference);
        assert_eq!(candidates[0].summary, "Prefers dark mode");
        assert_eq!(candidates[0].importance, 7.0);
        assert_eq!(candidates[0].source_type, SourceType::Implicit);
    }

    #[test]
    fn parse_multiple_candidates() {
        let response = r#"{"scope": "agent", "kind": "fact", "summary": "User is a Rust developer", "content": "10 years experience", "importance": 8}
{"scope": "user", "kind": "contact", "summary": "User name is Alice", "content": "Alice", "importance": 6}"#;
        let candidates = LlmMemoryExtractor::parse_candidates(response, &AgentId::new());
        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].kind, MemoryKind::Fact);
        assert_eq!(candidates[1].kind, MemoryKind::Contact);
    }

    #[test]
    fn parse_skips_non_json_lines() {
        let response = "Here are the extracted memories:\n{\"scope\": \"agent\", \"kind\": \"fact\", \"summary\": \"test\", \"content\": \"test\", \"importance\": 5}\nDone.";
        let candidates = LlmMemoryExtractor::parse_candidates(response, &AgentId::new());
        assert_eq!(candidates.len(), 1);
    }

    #[test]
    fn parse_clamps_importance() {
        let response = r#"{"scope": "agent", "kind": "fact", "summary": "test", "content": "test", "importance": 15}"#;
        let candidates = LlmMemoryExtractor::parse_candidates(response, &AgentId::new());
        assert_eq!(candidates[0].importance, 10.0);

        let response2 = r#"{"scope": "agent", "kind": "fact", "summary": "test", "content": "test", "importance": -5}"#;
        let candidates2 = LlmMemoryExtractor::parse_candidates(response2, &AgentId::new());
        assert_eq!(candidates2[0].importance, 1.0);
    }

    #[test]
    fn parse_defaults_missing_fields() {
        let response = r#"{"summary": "test"}"#;
        let candidates = LlmMemoryExtractor::parse_candidates(response, &AgentId::new());
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].scope, MemoryScope::Agent); // default
        assert_eq!(candidates[0].kind, MemoryKind::Fact); // default
        assert_eq!(candidates[0].importance, 5.0); // default
    }

    #[test]
    fn build_prompt_includes_messages() {
        let messages = vec![
            Message {
                role: MessageRole::User,
                content: "I prefer dark mode".to_string(),
                blocks: vec![],
                tool_call_id: None,
            },
            Message {
                role: MessageRole::Assistant,
                content: "Noted!".to_string(),
                blocks: vec![],
                tool_call_id: None,
            },
        ];
        let prompt = LlmMemoryExtractor::build_prompt(&messages);
        assert!(prompt.contains("User: I prefer dark mode"));
        assert!(prompt.contains("Assistant: Noted!"));
        assert!(prompt.contains("Extract"));
    }

    // -------------------------------------------------------------------
    // MemoryExtractor trait tests
    // -------------------------------------------------------------------

    #[tokio::test]
    async fn extractor_with_stub_llm() {
        let llm = Arc::new(clawx_llm::StubLlmProvider);
        let extractor = LlmMemoryExtractor::new(llm, "default".to_string());
        let messages = vec![Message {
            role: MessageRole::User,
            content: "Hello".to_string(),
            blocks: vec![],
            tool_call_id: None,
        }];
        // StubLlmProvider returns "[stub] response" which won't parse as JSON
        let candidates = extractor.extract(&AgentId::new(), &messages).await.unwrap();
        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn extractor_empty_messages_returns_empty() {
        let llm = Arc::new(clawx_llm::StubLlmProvider);
        let extractor = LlmMemoryExtractor::new(llm, "default".to_string());
        let candidates = extractor.extract(&AgentId::new(), &[]).await.unwrap();
        assert!(candidates.is_empty());
    }

    #[tokio::test]
    async fn stub_extractor_returns_empty() {
        let extractor = StubMemoryExtractor;
        let candidates = extractor.extract(&AgentId::new(), &[]).await.unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn extractor_is_object_safe() {
        fn _assert(_: Arc<dyn MemoryExtractor>) {}
    }
}
