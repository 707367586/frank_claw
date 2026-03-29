//! Task creation parser: extracts structured Task definitions from natural language.
//!
//! Uses LLM structured output to parse user requests like "remind me every morning
//! to check email" into a Task + Trigger configuration.

use clawx_types::autonomy::*;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::*;
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{debug, warn};

/// Structured schema for LLM to fill when parsing task creation requests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCreationRequest {
    /// Short task name (max 80 chars).
    pub name: String,
    /// Full goal description.
    pub goal: String,
    /// Trigger type: "time", "event", or "none".
    pub trigger_type: String,
    /// Cron expression if trigger_type is "time" (e.g., "0 0 9 * * *").
    #[serde(default)]
    pub cron: Option<String>,
    /// Event kind if trigger_type is "event".
    #[serde(default)]
    pub event_kind: Option<String>,
    /// Max steps for execution (default: 10).
    #[serde(default = "default_max_steps")]
    pub max_steps: u32,
    /// Whether the user confirmed the task.
    #[serde(default = "default_true")]
    pub is_valid_task: bool,
}

fn default_max_steps() -> u32 { 10 }
fn default_true() -> bool { true }

const TASK_PARSING_PROMPT: &str = r#"You are a task parser. Extract a structured task definition from the user's request.
Respond ONLY with a JSON object matching this schema:
{
  "name": "short task name (max 80 chars)",
  "goal": "full description of what the task should do",
  "trigger_type": "time" | "event" | "none",
  "cron": "6-field cron (sec min hour day month dow), e.g. '0 0 9 * * *' for daily at 9am",
  "event_kind": "event type if trigger_type is event",
  "max_steps": 10,
  "is_valid_task": true
}

If the user's message is NOT a task creation request, set is_valid_task to false.
Common patterns:
- "every morning" → cron: "0 0 9 * * *"
- "every hour" → cron: "0 0 * * * *"
- "every Monday" → cron: "0 0 9 * * 1"
- "daily at 3pm" → cron: "0 0 15 * * *"
- "when X happens" → trigger_type: "event", event_kind: "X""#;

/// Parse natural language into a TaskCreationRequest using LLM.
pub async fn parse_task_from_natural_language(
    llm: &Arc<dyn LlmProvider>,
    user_input: &str,
) -> Result<TaskCreationRequest> {
    let request = CompletionRequest {
        model: "default".to_string(),
        messages: vec![
            Message {
                role: MessageRole::System,
                content: TASK_PARSING_PROMPT.to_string(),
                tool_call_id: None,
            },
            Message {
                role: MessageRole::User,
                content: user_input.to_string(),
                tool_call_id: None,
            },
        ],
        tools: None,
        temperature: Some(0.0),
        max_tokens: Some(512),
        stream: false,
    };

    let response = llm.complete(request).await?;
    debug!(raw_response = %response.content, "LLM task parsing response");

    // Extract JSON from the response (handle markdown code blocks)
    let json_str = extract_json(&response.content);

    serde_json::from_str::<TaskCreationRequest>(json_str).map_err(|e| {
        warn!(error = %e, raw = %response.content, "failed to parse task from LLM response");
        ClawxError::Task(format!(
            "failed to parse task from LLM response: {}",
            e
        ))
    })
}

/// Fallback: heuristic-based task parsing (no LLM required).
/// Extracts task name, trigger type, and cron from common patterns.
pub fn parse_task_heuristic(user_input: &str) -> Option<TaskCreationRequest> {
    let lower = user_input.to_lowercase();

    // Check for task-creation intent keywords
    let task_keywords = [
        "remind me", "set a reminder", "schedule", "every day",
        "every morning", "every evening", "every hour", "every week",
        "create a task", "add a task", "set up", "automate",
    ];

    let has_task_intent = task_keywords.iter().any(|kw| lower.contains(kw));
    if !has_task_intent {
        return None;
    }

    // Extract cron pattern from common phrases
    let (trigger_type, cron) = if lower.contains("every morning") || lower.contains("every day at") {
        ("time".to_string(), Some("0 0 9 * * *".to_string()))
    } else if lower.contains("every evening") {
        ("time".to_string(), Some("0 0 18 * * *".to_string()))
    } else if lower.contains("every hour") {
        ("time".to_string(), Some("0 0 * * * *".to_string()))
    } else if lower.contains("every week") || lower.contains("every monday") {
        ("time".to_string(), Some("0 0 9 * * 1".to_string()))
    } else {
        ("none".to_string(), None)
    };

    // Name: truncate input (char-boundary safe for multi-byte UTF-8)
    let trimmed = user_input.trim();
    let name = if trimmed.chars().count() <= 80 {
        trimmed.to_string()
    } else {
        let truncated: String = trimmed.chars().take(77).collect();
        format!("{}...", truncated)
    };

    Some(TaskCreationRequest {
        name,
        goal: user_input.to_string(),
        trigger_type,
        cron,
        event_kind: None,
        max_steps: 10,
        is_valid_task: true,
    })
}

/// Build a Task + optional Trigger from a TaskCreationRequest.
pub fn build_task_and_trigger(
    agent_id: &AgentId,
    request: &TaskCreationRequest,
) -> (Task, Option<Trigger>) {
    let now = chrono::Utc::now();
    let task_id = TaskId::new();

    let task = Task {
        id: task_id,
        agent_id: *agent_id,
        name: request.name.clone(),
        goal: request.goal.clone(),
        source_kind: TaskSourceKind::Conversation,
        lifecycle_status: TaskLifecycleStatus::Active,
        default_max_steps: request.max_steps,
        default_timeout_secs: 300,
        notification_policy: serde_json::json!({}),
        suppression_state: SuppressionState::default(),
        last_run_at: None,
        next_run_at: None,
        created_at: now,
        updated_at: now,
    };

    let trigger = match request.trigger_type.as_str() {
        "time" => {
            let trigger_config = serde_json::json!({
                "cron": request.cron.as_deref().unwrap_or("0 0 9 * * *"),
            });
            // Compute next_fire_at from cron
            let next_fire = request.cron.as_ref().and_then(|cron_str| {
                clawx_scheduler::next_fire_time(cron_str, now).ok()
            });

            Some(Trigger {
                id: TriggerId::new(),
                task_id,
                trigger_kind: TriggerKind::Time,
                trigger_config,
                status: TriggerStatus::Active,
                next_fire_at: next_fire,
                last_fired_at: None,
                created_at: now,
                updated_at: now,
            })
        }
        "event" => {
            let trigger_config = serde_json::json!({
                "event_kind": request.event_kind.as_deref().unwrap_or("unknown"),
            });
            Some(Trigger {
                id: TriggerId::new(),
                task_id,
                trigger_kind: TriggerKind::Event,
                trigger_config,
                status: TriggerStatus::Active,
                next_fire_at: None,
                last_fired_at: None,
                created_at: now,
                updated_at: now,
            })
        }
        _ => None,
    };

    (task, trigger)
}

/// Extract JSON from LLM response, handling markdown code blocks.
fn extract_json(text: &str) -> &str {
    let trimmed = text.trim();
    // Handle ```json ... ``` blocks
    if let Some(start) = trimmed.find("```json") {
        let json_start = start + 7;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    // Handle ``` ... ``` blocks
    if let Some(start) = trimmed.find("```") {
        let json_start = start + 3;
        if let Some(end) = trimmed[json_start..].find("```") {
            return trimmed[json_start..json_start + end].trim();
        }
    }
    // Try the whole thing as JSON
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heuristic_parses_daily_reminder() {
        let result = parse_task_heuristic("Remind me every morning to check email").unwrap();
        assert_eq!(result.trigger_type, "time");
        assert_eq!(result.cron.as_deref(), Some("0 0 9 * * *"));
        assert!(result.is_valid_task);
        assert!(result.name.contains("Remind me"));
    }

    #[test]
    fn heuristic_parses_weekly_task() {
        let result = parse_task_heuristic("Schedule a weekly review every Monday").unwrap();
        assert_eq!(result.trigger_type, "time");
        assert_eq!(result.cron.as_deref(), Some("0 0 9 * * 1"));
    }

    #[test]
    fn heuristic_parses_hourly_task() {
        let result = parse_task_heuristic("Remind me every hour to drink water").unwrap();
        assert_eq!(result.trigger_type, "time");
        assert_eq!(result.cron.as_deref(), Some("0 0 * * * *"));
    }

    #[test]
    fn heuristic_rejects_non_task() {
        let result = parse_task_heuristic("What is the weather today?");
        assert!(result.is_none());
    }

    #[test]
    fn heuristic_parses_generic_task() {
        let result = parse_task_heuristic("Create a task to backup my files").unwrap();
        assert_eq!(result.trigger_type, "none");
        assert!(result.cron.is_none());
        assert!(result.is_valid_task);
    }

    #[test]
    fn build_task_creates_time_trigger() {
        let agent_id = AgentId::new();
        let req = TaskCreationRequest {
            name: "Daily email check".into(),
            goal: "Check email every morning".into(),
            trigger_type: "time".into(),
            cron: Some("0 0 9 * * *".into()),
            event_kind: None,
            max_steps: 5,
            is_valid_task: true,
        };

        let (task, trigger) = build_task_and_trigger(&agent_id, &req);
        assert_eq!(task.name, "Daily email check");
        assert_eq!(task.default_max_steps, 5);
        assert!(trigger.is_some());
        let t = trigger.unwrap();
        assert_eq!(t.trigger_kind, TriggerKind::Time);
        assert!(t.next_fire_at.is_some());
    }

    #[test]
    fn build_task_creates_event_trigger() {
        let agent_id = AgentId::new();
        let req = TaskCreationRequest {
            name: "On file change".into(),
            goal: "React to file changes".into(),
            trigger_type: "event".into(),
            cron: None,
            event_kind: Some("file_changed".into()),
            max_steps: 3,
            is_valid_task: true,
        };

        let (_, trigger) = build_task_and_trigger(&agent_id, &req);
        assert!(trigger.is_some());
        let t = trigger.unwrap();
        assert_eq!(t.trigger_kind, TriggerKind::Event);
    }

    #[test]
    fn build_task_no_trigger() {
        let agent_id = AgentId::new();
        let req = TaskCreationRequest {
            name: "One-shot task".into(),
            goal: "Do something once".into(),
            trigger_type: "none".into(),
            cron: None,
            event_kind: None,
            max_steps: 10,
            is_valid_task: true,
        };

        let (task, trigger) = build_task_and_trigger(&agent_id, &req);
        assert_eq!(task.source_kind, TaskSourceKind::Conversation);
        assert!(trigger.is_none());
    }

    #[test]
    fn extract_json_from_markdown_block() {
        let input = "Here's the result:\n```json\n{\"name\": \"test\"}\n```\nDone.";
        assert_eq!(extract_json(input), "{\"name\": \"test\"}");
    }

    #[test]
    fn extract_json_raw() {
        let input = "{\"name\": \"test\"}";
        assert_eq!(extract_json(input), "{\"name\": \"test\"}");
    }

    #[test]
    fn task_creation_request_deserialization() {
        let json = r#"{"name":"test","goal":"do it","trigger_type":"none","is_valid_task":true}"#;
        let req: TaskCreationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "test");
        assert_eq!(req.max_steps, 10); // default
        assert!(req.is_valid_task);
    }

    #[test]
    fn long_input_name_truncated() {
        let long_input = "Remind me ".to_string() + &"x".repeat(200);
        let result = parse_task_heuristic(&long_input).unwrap();
        assert!(result.name.len() <= 80);
        assert!(result.name.ends_with("..."));
    }
}
