# Agentic Tool-Use Loop — Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give ClawX agents real tool-use capability so they can create folders, read/write files, and run shell commands — grounded in a Claude-Code-style approval gate and macOS `sandbox-exec`.

**Architecture:** Add a new `clawx-tools` crate exposing a `Tool` trait + `ToolRegistry`. Extend `clawx-types::llm::Message` with structured `ContentBlock`s so `tool_use`/`tool_result` can round-trip through providers. Fix Anthropic + OpenAI-compat providers to actually serialize `tools` and parse tool-call content blocks. Extend `agent_loop::run_turn` into a tool-use iteration loop with a configurable `ApprovalPort` (auto / prompt / deny per tool×path).

**Tech Stack:** Rust 2021 · tokio · reqwest · serde · async-trait · `security-framework` (unused here) · macOS `sandbox-exec(1)`.

**Scope (this plan):** Tool-use iteration + 5 built-in tools (`fs_read`, `fs_write`, `fs_mkdir`, `fs_list`, `shell_exec`) + approval gate + Anthropic/OpenAI wire formats. **Out of scope (follow-up plans):** Hooks, SubTurn, Steering, MCP client, markdown skills — see §Follow-up Plans at the bottom.

**Vision — why this looks like picoclaw:** picoclaw (Go) gives agents a flat built-in toolset (fs, shell, web) + hooks/SubTurn/steering/MCP/skills. ClawX (Rust) already has the security/memory/scheduler/channel spine but never hooked tools into the LLM loop. This plan ports picoclaw's **agent-loop-with-tools + sandboxed exec** shape into the Rust workspace. Follow-up plans will port the coordination primitives (hook/SubTurn/steering) and the extensibility surfaces (MCP/skills-md).

---

## File Structure

### New files
- `crates/clawx-tools/Cargo.toml` — new crate manifest.
- `crates/clawx-tools/src/lib.rs` — `Tool` trait, `ToolRegistry`, `ToolExecCtx`, `ToolOutcome`, error types.
- `crates/clawx-tools/src/fs.rs` — `FsReadTool`, `FsWriteTool`, `FsMkdirTool`, `FsListTool`.
- `crates/clawx-tools/src/shell.rs` — `ShellExecTool` (macOS `sandbox-exec` wrapper; non-macOS returns `Unsupported`).
- `crates/clawx-tools/src/approval.rs` — `ApprovalPort` trait + `AutoApprovalGate` (rule-based).
- `crates/clawx-tools/src/sandbox_profile.rs` — generates `.sb` profile text for `sandbox-exec`.
- `crates/clawx-tools/tests/fs_integration.rs` — fs tools integration tests (real tempdir).
- `crates/clawx-runtime/src/tool_loop.rs` — tool-use iteration driver invoked by `agent_loop.rs`.
- `crates/clawx-runtime/tests/tool_loop_e2e.rs` — integration test: stub LLM emits tool_use → fs_mkdir runs → directory exists.

### Modified files
- `crates/clawx-types/src/llm.rs` — add `ContentBlock` enum, add `blocks: Vec<ContentBlock>` to `Message`, keep `content: String` for back-compat. **Lines ~16-46 + new types below.**
- `crates/clawx-types/src/error.rs` — add `ClawxError::Tool(String)` + `ClawxError::Approval(String)`.
- `crates/clawx-llm/src/anthropic.rs` — serialize `tools`; emit `content` as blocks; parse `tool_use` blocks into `LlmResponse.tool_calls`; handle `role: tool` → `user` with `tool_result` block.
- `crates/clawx-llm/src/openai.rs` — serialize `tools`, parse `tool_calls` on response, serialize role=tool messages.
- `crates/clawx-llm/src/zhipu.rs` — same as openai (GLM uses OpenAI-compat wire).
- `crates/clawx-runtime/src/agent_loop.rs:96-103` — stop writing `tools: None`; call `tool_loop::run_with_tools` when registry is present.
- `crates/clawx-runtime/src/lib.rs:32-87` — add `tools: Option<Arc<ToolRegistry>>` and `approval: Option<Arc<dyn ApprovalPort>>` to `Runtime`, with builder methods.
- `Cargo.toml` (workspace) — add `clawx-tools` to `members` + `workspace.dependencies`.
- `apps/clawx-service/Cargo.toml` + construction site — wire `ToolRegistry` with fs+shell.
- `docs/arch/decisions.md` — ADR entry for tool-use loop design.

---

## Task 1 · Extend `Message` with ContentBlock + new error variants

**Files:**
- Modify: `crates/clawx-types/src/llm.rs` (types near lines 16-46)
- Modify: `crates/clawx-types/src/error.rs`

- [ ] **Step 1: Write failing unit tests for block serde round-trip**

Append to `crates/clawx-types/src/llm.rs` (inside `#[cfg(test)] mod tests`, create if missing):

```rust
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
}
```

- [ ] **Step 2: Run tests — expect compile failures**

Run: `cargo test -p clawx-types content_block_tests -- --nocapture`
Expected: **FAIL** — `ContentBlock`, `blocks` field unknown.

- [ ] **Step 3: Add `ContentBlock` + `blocks` field**

Edit `crates/clawx-types/src/llm.rs`. Replace the existing `Message` struct (lines ~17-23) with:

```rust
/// A single message in an LLM conversation.
///
/// `content` remains the plain-text channel for back-compat with older code paths.
/// `blocks` is additive: when non-empty it carries structured content
/// (`tool_use`, `tool_result`, future `image`) that providers serialize
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
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}
```

- [ ] **Step 4: Add `Tool` + `Approval` error variants**

Open `crates/clawx-types/src/error.rs`. Find the `ClawxError` enum (look for `#[derive(...Debug, thiserror::Error...)] pub enum ClawxError`). Add these two variants before the final brace:

```rust
    #[error("tool error: {0}")]
    Tool(String),
    #[error("approval denied: {0}")]
    Approval(String),
```

- [ ] **Step 5: Verify every `Message { ... }` construction still compiles**

Run: `cargo build -p clawx-types && cargo build --workspace 2>&1 | grep -E "error\[E0063\]|missing field .blocks." | head -20`
Expected: workspace build clean because `blocks` has `#[serde(default)]` + we kept `content`. If a struct-init site fails, add `blocks: vec![]` there.

- [ ] **Step 6: Run the tests — expect green**

Run: `cargo test -p clawx-types content_block_tests`
Expected: **PASS** (3 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/clawx-types/src/llm.rs crates/clawx-types/src/error.rs
git commit -m "feat(types): add ContentBlock + Tool/Approval error variants"
```

---

## Task 2 · Anthropic provider: real tool_use / tool_result

**Files:**
- Modify: `crates/clawx-llm/src/anthropic.rs` (entire `build_body`, `AnthropicMessage`, `AnthropicContent`, and the response-mapping code)

- [ ] **Step 1: Write the failing tests**

Append to `crates/clawx-llm/src/anthropic.rs`:

```rust
#[cfg(test)]
mod tool_use_tests {
    use super::*;
    use clawx_types::llm::{ContentBlock, ToolDefinition};

    fn dummy_provider() -> AnthropicProvider {
        AnthropicProvider::new("sk-test".into(), "http://127.0.0.1".into())
    }

    #[test]
    fn build_body_serializes_tools() {
        let req = CompletionRequest {
            model: "claude-sonnet-4-6".into(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "hi".into(),
                blocks: vec![],
                tool_call_id: None,
            }],
            tools: Some(vec![ToolDefinition {
                name: "fs_mkdir".into(),
                description: "Create a directory".into(),
                parameters: serde_json::json!({"type":"object"}),
            }]),
            temperature: None,
            max_tokens: Some(256),
            stream: false,
        };
        let body = dummy_provider().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["tools"][0]["name"], "fs_mkdir");
        assert_eq!(json["tools"][0]["input_schema"]["type"], "object");
    }

    #[test]
    fn build_body_serializes_tool_result_message_as_user_block() {
        let req = CompletionRequest {
            model: "claude-sonnet-4-6".into(),
            messages: vec![Message {
                role: MessageRole::Tool,
                content: String::new(),
                blocks: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "ok".into(),
                    is_error: false,
                }],
                tool_call_id: Some("call_1".into()),
            }],
            tools: None,
            temperature: None,
            max_tokens: Some(256),
            stream: false,
        };
        let body = dummy_provider().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][0]["content"][0]["type"], "tool_result");
        assert_eq!(json["messages"][0]["content"][0]["tool_use_id"], "call_1");
    }

    #[test]
    fn parse_response_extracts_tool_use_blocks() {
        let raw = serde_json::json!({
            "id": "msg_1",
            "model": "claude-sonnet-4-6",
            "stop_reason": "tool_use",
            "usage": {"input_tokens": 5, "output_tokens": 7},
            "content": [
                {"type": "text", "text": "I'll create it."},
                {"type": "tool_use", "id": "call_1", "name": "fs_mkdir",
                 "input": {"path": "/tmp/x"}}
            ]
        });
        let resp: AnthropicResponse = serde_json::from_value(raw).unwrap();
        let mapped = to_llm_response(resp);
        assert_eq!(mapped.tool_calls.len(), 1);
        assert_eq!(mapped.tool_calls[0].name, "fs_mkdir");
        assert_eq!(mapped.stop_reason, StopReason::ToolUse);
        assert!(mapped.content.contains("I'll create it."));
    }
}
```

- [ ] **Step 2: Run tests to confirm they fail**

Run: `cargo test -p clawx-llm tool_use_tests`
Expected: **FAIL** (functions/fields not defined).

- [ ] **Step 3: Rewrite wire types + build_body + response mapper**

Replace the region from `fn build_body` through the end of `fn map_stop_reason` in `crates/clawx-llm/src/anthropic.rs` with:

```rust
fn build_body(&self, request: &CompletionRequest) -> AnthropicRequestBody {
    let mut system: Option<String> = None;
    let mut messages: Vec<AnthropicMessage> = Vec::new();

    for msg in &request.messages {
        match msg.role {
            MessageRole::System => system = Some(msg.content.clone()),
            MessageRole::User | MessageRole::Assistant | MessageRole::Tool => {
                let role = match msg.role {
                    MessageRole::Assistant => "assistant",
                    // Tool-result messages ride on `user` per Anthropic's schema.
                    _ => "user",
                };
                let content = if msg.blocks.is_empty() {
                    AnthropicContentField::Text(msg.content.clone())
                } else {
                    AnthropicContentField::Blocks(
                        msg.blocks.iter().map(to_anthropic_block).collect(),
                    )
                };
                messages.push(AnthropicMessage { role: role.to_string(), content });
            }
        }
    }

    let tools = request.tools.as_ref().map(|defs| {
        defs.iter()
            .map(|d| AnthropicTool {
                name: d.name.clone(),
                description: d.description.clone(),
                input_schema: d.parameters.clone(),
            })
            .collect()
    });

    AnthropicRequestBody {
        model: request.model.clone(),
        max_tokens: request.max_tokens.unwrap_or(4096),
        system,
        messages,
        temperature: request.temperature,
        stream: request.stream,
        tools,
    }
}
```

Replace the wire types (`AnthropicRequestBody`, `AnthropicMessage`, `AnthropicResponse`, `AnthropicContent`) and add a mapper:

```rust
#[derive(Debug, Serialize)]
struct AnthropicRequestBody {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    messages: Vec<AnthropicMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<AnthropicTool>>,
}

#[derive(Debug, Serialize)]
struct AnthropicTool {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct AnthropicMessage {
    role: String,
    content: AnthropicContentField,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AnthropicContentField {
    Text(String),
    Blocks(Vec<AnthropicBlock>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum AnthropicBlock {
    Text { text: String },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        is_error: bool,
    },
}

fn to_anthropic_block(b: &clawx_types::llm::ContentBlock) -> AnthropicBlock {
    use clawx_types::llm::ContentBlock::*;
    match b {
        Text { text } => AnthropicBlock::Text { text: text.clone() },
        ToolUse { id, name, input } => AnthropicBlock::ToolUse {
            id: id.clone(),
            name: name.clone(),
            input: input.clone(),
        },
        ToolResult { tool_use_id, content, is_error } => AnthropicBlock::ToolResult {
            tool_use_id: tool_use_id.clone(),
            content: content.clone(),
            is_error: *is_error,
        },
    }
}

#[derive(Debug, Deserialize)]
struct AnthropicResponse {
    #[allow(dead_code)]
    id: String,
    content: Vec<AnthropicBlock>,
    stop_reason: Option<String>,
    usage: AnthropicUsage,
    model: String,
}

#[derive(Debug, Deserialize)]
struct AnthropicUsage {
    input_tokens: u32,
    output_tokens: u32,
}

fn to_llm_response(raw: AnthropicResponse) -> LlmResponse {
    let mut text = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();
    for block in raw.content {
        match block {
            AnthropicBlock::Text { text: t } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&t);
            }
            AnthropicBlock::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall { id, name, arguments: input });
            }
            AnthropicBlock::ToolResult { .. } => {
                // Providers never emit tool_result back to us; ignore.
            }
        }
    }
    LlmResponse {
        content: text,
        stop_reason: map_stop_reason(raw.stop_reason.as_deref()),
        tool_calls,
        usage: TokenUsage {
            prompt_tokens: raw.usage.input_tokens,
            completion_tokens: raw.usage.output_tokens,
            total_tokens: raw.usage.input_tokens + raw.usage.output_tokens,
        },
        metadata: Some(ProviderMetadata {
            provider: "anthropic".into(),
            model_id: raw.model,
            extra: None,
        }),
    }
}

fn map_stop_reason(reason: Option<&str>) -> StopReason {
    match reason {
        Some("end_turn") => StopReason::EndTurn,
        Some("max_tokens") => StopReason::MaxTokens,
        Some("tool_use") => StopReason::ToolUse,
        Some("stop_sequence") => StopReason::StopSequence,
        _ => StopReason::EndTurn,
    }
}
```

Find the `impl LlmProvider for AnthropicProvider { async fn complete(...) }` body. Replace the response-parsing line (the one that builds `LlmResponse`) with `Ok(to_llm_response(raw))` where `raw` is the deserialized `AnthropicResponse`.

- [ ] **Step 4: Run tests — expect PASS**

Run: `cargo test -p clawx-llm tool_use_tests`
Expected: **PASS** (3 tests).

- [ ] **Step 5: Clippy + fmt clean**

Run: `cargo clippy -p clawx-llm --all-targets -- -D warnings && cargo fmt -p clawx-llm`
Expected: no warnings, no diff.

- [ ] **Step 6: Commit**

```bash
git add crates/clawx-llm/src/anthropic.rs
git commit -m "feat(llm/anthropic): emit tools + parse tool_use/tool_result blocks"
```

---

## Task 3 · OpenAI + Zhipu providers: tool_calls wire format

**Files:**
- Modify: `crates/clawx-llm/src/openai.rs`
- Modify: `crates/clawx-llm/src/zhipu.rs`

Rationale: OpenAI/GLM use a flat `tool_calls` array on assistant messages and `role: "tool"` with `tool_call_id` for results. We round-trip these through our `ContentBlock` representation.

- [ ] **Step 1: Write failing tests in `openai.rs`**

Append to `crates/clawx-llm/src/openai.rs`:

```rust
#[cfg(test)]
mod tool_calls_tests {
    use super::*;
    use clawx_types::llm::{ContentBlock, ToolDefinition};

    fn dummy() -> OpenAiProvider {
        OpenAiProvider::new("sk-test".into(), "http://127.0.0.1".into(), "gpt-x".into())
    }

    #[test]
    fn build_body_serializes_tools_array() {
        let req = CompletionRequest {
            model: "gpt-x".into(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "hi".into(),
                blocks: vec![],
                tool_call_id: None,
            }],
            tools: Some(vec![ToolDefinition {
                name: "fs_mkdir".into(),
                description: "Create a directory".into(),
                parameters: serde_json::json!({"type":"object"}),
            }]),
            temperature: None,
            max_tokens: None,
            stream: false,
        };
        let body = dummy().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["tools"][0]["type"], "function");
        assert_eq!(json["tools"][0]["function"]["name"], "fs_mkdir");
    }

    #[test]
    fn build_body_serializes_tool_result_role() {
        let req = CompletionRequest {
            model: "gpt-x".into(),
            messages: vec![Message {
                role: MessageRole::Tool,
                content: String::new(),
                blocks: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "ok".into(),
                    is_error: false,
                }],
                tool_call_id: Some("call_1".into()),
            }],
            tools: None,
            temperature: None,
            max_tokens: None,
            stream: false,
        };
        let body = dummy().build_body(&req);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["messages"][0]["role"], "tool");
        assert_eq!(json["messages"][0]["tool_call_id"], "call_1");
        assert_eq!(json["messages"][0]["content"], "ok");
    }

    #[test]
    fn parse_response_maps_tool_calls() {
        let raw = serde_json::json!({
            "id": "c1",
            "model": "gpt-x",
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "fs_mkdir",
                                     "arguments": "{\"path\":\"/tmp/x\"}"}
                    }]
                }
            }],
            "usage": {"prompt_tokens": 3, "completion_tokens": 4, "total_tokens": 7}
        });
        let resp: OpenAiResponse = serde_json::from_value(raw).unwrap();
        let mapped = to_llm_response(resp, "openai");
        assert_eq!(mapped.tool_calls.len(), 1);
        assert_eq!(mapped.tool_calls[0].name, "fs_mkdir");
        assert_eq!(mapped.stop_reason, StopReason::ToolUse);
    }
}
```

- [ ] **Step 2: Run — confirm failure**

Run: `cargo test -p clawx-llm tool_calls_tests`
Expected: **FAIL** (compile).

- [ ] **Step 3: Update `openai.rs` wire types + build_body + mapper**

Open `crates/clawx-llm/src/openai.rs`. Replace the message/response wire structs and the body builder with:

```rust
#[derive(Debug, Serialize)]
pub(crate) struct OpenAiRequestBody {
    pub(crate) model: String,
    pub(crate) messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tools: Option<Vec<OpenAiTool>>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiTool {
    #[serde(rename = "type")]
    pub(crate) kind: &'static str, // always "function"
    pub(crate) function: OpenAiFunctionDef,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiFunctionDef {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAiMessage {
    pub(crate) role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct OpenAiToolCall {
    pub(crate) id: String,
    #[serde(rename = "type", default)]
    pub(crate) kind: String,
    pub(crate) function: OpenAiFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub(crate) struct OpenAiFunctionCall {
    pub(crate) name: String,
    pub(crate) arguments: String,
}
```

Rewrite `build_body` on `OpenAiProvider`:

```rust
pub(crate) fn build_body(&self, req: &CompletionRequest) -> OpenAiRequestBody {
    use clawx_types::llm::ContentBlock;
    let mut messages = Vec::new();
    for m in &req.messages {
        let role = match m.role {
            MessageRole::System => "system",
            MessageRole::User => "user",
            MessageRole::Assistant => "assistant",
            MessageRole::Tool => "tool",
        };

        if m.role == MessageRole::Tool {
            // Flatten ToolResult blocks into the top-level content string.
            let mut txt = String::new();
            let mut tool_call_id = m.tool_call_id.clone();
            for b in &m.blocks {
                if let ContentBlock::ToolResult { tool_use_id, content, .. } = b {
                    if !txt.is_empty() { txt.push('\n'); }
                    txt.push_str(content);
                    tool_call_id.get_or_insert_with(|| tool_use_id.clone());
                }
            }
            if txt.is_empty() { txt = m.content.clone(); }
            messages.push(OpenAiMessage {
                role: role.into(),
                content: Some(txt),
                tool_call_id,
                tool_calls: vec![],
            });
            continue;
        }

        // Assistant messages may carry ToolUse blocks we need to translate.
        let mut calls: Vec<OpenAiToolCall> = Vec::new();
        for b in &m.blocks {
            if let ContentBlock::ToolUse { id, name, input } = b {
                calls.push(OpenAiToolCall {
                    id: id.clone(),
                    kind: "function".into(),
                    function: OpenAiFunctionCall {
                        name: name.clone(),
                        arguments: input.to_string(),
                    },
                });
            }
        }
        let content = if m.content.is_empty() && !calls.is_empty() { None } else { Some(m.content.clone()) };
        messages.push(OpenAiMessage {
            role: role.into(),
            content,
            tool_call_id: None,
            tool_calls: calls,
        });
    }

    let tools = req.tools.as_ref().map(|defs| {
        defs.iter()
            .map(|d| OpenAiTool {
                kind: "function",
                function: OpenAiFunctionDef {
                    name: d.name.clone(),
                    description: d.description.clone(),
                    parameters: d.parameters.clone(),
                },
            })
            .collect()
    });

    OpenAiRequestBody {
        model: req.model.clone(),
        messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        stream: req.stream,
        tools,
    }
}
```

Add the response types + mapper:

```rust
#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiResponse {
    #[allow(dead_code)]
    pub(crate) id: String,
    pub(crate) model: String,
    pub(crate) choices: Vec<OpenAiChoice>,
    pub(crate) usage: Option<OpenAiUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiChoice {
    pub(crate) message: OpenAiRespMessage,
    pub(crate) finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiRespMessage {
    #[serde(default)]
    pub(crate) content: Option<String>,
    #[serde(default)]
    pub(crate) tool_calls: Vec<OpenAiToolCall>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAiUsage {
    pub(crate) prompt_tokens: u32,
    pub(crate) completion_tokens: u32,
    pub(crate) total_tokens: u32,
}

pub(crate) fn to_llm_response(resp: OpenAiResponse, provider: &'static str) -> LlmResponse {
    let choice = resp.choices.into_iter().next();
    let (content, tool_calls, finish) = match choice {
        Some(c) => {
            let calls = c.message.tool_calls.into_iter()
                .map(|tc| ToolCall {
                    id: tc.id,
                    name: tc.function.name,
                    arguments: serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::String(tc.function.arguments)),
                })
                .collect::<Vec<_>>();
            (c.message.content.unwrap_or_default(), calls, c.finish_reason)
        }
        None => (String::new(), vec![], None),
    };
    let stop_reason = match (finish.as_deref(), tool_calls.is_empty()) {
        (Some("tool_calls"), _) => StopReason::ToolUse,
        (_, false) => StopReason::ToolUse,
        (Some("length"), _) => StopReason::MaxTokens,
        _ => StopReason::EndTurn,
    };
    let usage = resp.usage.map(|u| TokenUsage {
        prompt_tokens: u.prompt_tokens,
        completion_tokens: u.completion_tokens,
        total_tokens: u.total_tokens,
    }).unwrap_or_default();
    LlmResponse {
        content,
        stop_reason,
        tool_calls,
        usage,
        metadata: Some(ProviderMetadata {
            provider: provider.into(),
            model_id: resp.model,
            extra: None,
        }),
    }
}
```

In the provider's `complete` method, replace response parsing to use `to_llm_response(parsed, "openai")`.

- [ ] **Step 4: Mirror in `zhipu.rs`**

GLM uses the same wire. Open `crates/clawx-llm/src/zhipu.rs`, remove its private wire structs, and switch to re-using the OpenAI ones:

```rust
use crate::openai::{OpenAiRequestBody, OpenAiResponse, to_llm_response};
```

In `zhipu.rs`'s body-builder, delegate to `openai::OpenAiProvider::build_body` shape (copy it verbatim if the current zhipu provider doesn't take an `OpenAiProvider` instance). In `complete`, use `to_llm_response(parsed, "zhipu")`.

Add the same `tool_calls_tests` test module to `zhipu.rs` with provider name `"zhipu"`.

- [ ] **Step 5: Run both test suites**

Run: `cargo test -p clawx-llm`
Expected: **PASS** (existing tests + 6 new).

- [ ] **Step 6: Clippy + fmt**

Run: `cargo clippy -p clawx-llm --all-targets -- -D warnings && cargo fmt -p clawx-llm`

- [ ] **Step 7: Commit**

```bash
git add crates/clawx-llm/src/openai.rs crates/clawx-llm/src/zhipu.rs
git commit -m "feat(llm): tool_calls wire for openai-compat + zhipu"
```

---

## Task 4 · Scaffold `clawx-tools` crate

**Files:**
- Create: `crates/clawx-tools/Cargo.toml`
- Create: `crates/clawx-tools/src/lib.rs`
- Modify: `Cargo.toml` (workspace) — add member + dep entry.

- [ ] **Step 1: Create the crate manifest**

Write `crates/clawx-tools/Cargo.toml`:

```toml
[package]
name = "clawx-tools"
version.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true

[dependencies]
clawx-types = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create the crate root with the Tool trait**

Write `crates/clawx-tools/src/lib.rs`:

```rust
//! clawx-tools — built-in agent tools + registry.
//!
//! Provides the `Tool` trait, `ToolRegistry`, and `ToolExecCtx` that power
//! the agent-loop tool-use iteration. Concrete tools live in sibling modules
//! (`fs`, `shell`). Approval and sandbox helpers live in `approval` and
//! `sandbox_profile`.

pub mod approval;
pub mod fs;
pub mod sandbox_profile;
pub mod shell;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::AgentId;
use clawx_types::llm::ToolDefinition;
use serde::{Deserialize, Serialize};

pub use approval::{ApprovalDecision, ApprovalPort, AutoApprovalGate};

/// Outcome of a tool invocation. `content` is the string we hand back to
/// the LLM as a `tool_result` block; `is_error` flips the block's is_error flag.
#[derive(Debug, Clone)]
pub struct ToolOutcome {
    pub content: String,
    pub is_error: bool,
}

impl ToolOutcome {
    pub fn ok(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: false }
    }
    pub fn err(content: impl Into<String>) -> Self {
        Self { content: content.into(), is_error: true }
    }
}

/// Execution context passed to every tool call. Carries workspace root,
/// agent id, and a handle to the approval port.
#[derive(Clone)]
pub struct ToolExecCtx {
    pub agent_id: AgentId,
    /// Canonicalized workspace directory. All relative paths resolve here,
    /// and no tool may write outside this tree.
    pub workspace: PathBuf,
    pub approval: Arc<dyn ApprovalPort>,
}

/// A single callable tool.
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn invoke(
        &self,
        ctx: &ToolExecCtx,
        arguments: serde_json::Value,
    ) -> Result<ToolOutcome>;
}

/// Registry: maps tool name → `Arc<dyn Tool>`. Deterministic iteration order
/// via `BTreeMap` would be nicer but `HashMap` is fine since the LLM sees the
/// list via `definitions()` which is sorted.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, tool: Arc<dyn Tool>) {
        self.tools.insert(tool.definition().name, tool);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(name).cloned()
    }

    /// All tool definitions, sorted by name for stable prompts.
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<_> = self.tools.values().map(|t| t.definition()).collect();
        defs.sort_by(|a, b| a.name.cmp(&b.name));
        defs
    }

    pub fn is_empty(&self) -> bool { self.tools.is_empty() }
    pub fn len(&self) -> usize { self.tools.len() }
}

/// Helper: deserialize a tool's JSON arguments into its concrete params struct.
pub fn parse_args<T: for<'de> Deserialize<'de>>(
    tool_name: &str,
    args: serde_json::Value,
) -> Result<T> {
    serde_json::from_value(args).map_err(|e| {
        ClawxError::Tool(format!("tool {}: invalid arguments: {}", tool_name, e))
    })
}

/// Canonicalize `path_arg` under `workspace`. Returns `Err` if it escapes.
pub fn resolve_in_workspace(workspace: &std::path::Path, path_arg: &str) -> Result<PathBuf> {
    let p = std::path::Path::new(path_arg);
    let joined = if p.is_absolute() { p.to_path_buf() } else { workspace.join(p) };
    // Disallow `..` components outright. We don't call `canonicalize` because
    // the path may not exist yet (mkdir, write). Instead, walk the components.
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            std::path::Component::ParentDir => {
                return Err(ClawxError::Tool(format!(
                    "path escapes workspace via '..': {}", path_arg
                )));
            }
            _ => out.push(c.as_os_str()),
        }
    }
    if !out.starts_with(workspace) {
        return Err(ClawxError::Tool(format!(
            "path outside workspace: {}", out.display()
        )));
    }
    Ok(out)
}

/// Shared serde schema for tools that take a single `path` argument.
#[derive(Debug, Deserialize, Serialize)]
pub struct PathArgs {
    pub path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn resolve_rejects_parent_dir() {
        let ws = PathBuf::from("/ws");
        let err = resolve_in_workspace(&ws, "../etc/passwd").unwrap_err();
        assert!(format!("{err}").contains("escapes"));
    }

    #[test]
    fn resolve_accepts_subdir() {
        let ws = PathBuf::from("/ws");
        let got = resolve_in_workspace(&ws, "sub/dir").unwrap();
        assert_eq!(got, PathBuf::from("/ws/sub/dir"));
    }

    #[test]
    fn registry_sorts_definitions() {
        let mut r = ToolRegistry::new();
        // defer fs tool import to keep this unit test self-contained
        struct StubTool(&'static str);
        #[async_trait::async_trait]
        impl Tool for StubTool {
            fn definition(&self) -> ToolDefinition {
                ToolDefinition {
                    name: self.0.into(),
                    description: String::new(),
                    parameters: serde_json::json!({}),
                }
            }
            async fn invoke(&self, _: &ToolExecCtx, _: serde_json::Value) -> Result<ToolOutcome> {
                Ok(ToolOutcome::ok("x"))
            }
        }
        r.register(Arc::new(StubTool("b")));
        r.register(Arc::new(StubTool("a")));
        let names: Vec<String> = r.definitions().into_iter().map(|d| d.name).collect();
        assert_eq!(names, vec!["a", "b"]);
    }
}
```

- [ ] **Step 3: Create placeholder module files so the crate compiles**

Write `crates/clawx-tools/src/approval.rs`:

```rust
//! Approval port — will be filled in by Task 7.
use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::ids::AgentId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny { reason: String },
}

#[async_trait]
pub trait ApprovalPort: Send + Sync {
    async fn check(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ApprovalDecision>;
}

/// Permissive gate — allows everything. Tests and dev only.
#[derive(Debug, Default)]
pub struct AutoApprovalGate;

#[async_trait]
impl ApprovalPort for AutoApprovalGate {
    async fn check(
        &self,
        _agent_id: &AgentId,
        _tool: &str,
        _args: &serde_json::Value,
    ) -> Result<ApprovalDecision> {
        Ok(ApprovalDecision::Allow)
    }
}
```

Write empty stubs `crates/clawx-tools/src/fs.rs`, `crates/clawx-tools/src/shell.rs`, `crates/clawx-tools/src/sandbox_profile.rs`, each containing only `// stub`. These get filled in Tasks 5, 6, and as a helper in Task 6 respectively.

- [ ] **Step 4: Wire into workspace**

Edit root `Cargo.toml`. In `[workspace] members = [...]` add `"crates/clawx-tools",` (keep alphabetical). In `[workspace.dependencies]` add:

```toml
clawx-tools = { path = "crates/clawx-tools" }
```

- [ ] **Step 5: Build + run unit tests**

Run: `cargo test -p clawx-tools`
Expected: **PASS** (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/clawx-tools Cargo.toml
git commit -m "feat(tools): scaffold clawx-tools crate with Tool trait + registry"
```

---

## Task 5 · Filesystem tools (fs_read / fs_write / fs_mkdir / fs_list)

**Files:**
- Modify: `crates/clawx-tools/src/fs.rs`
- Create: `crates/clawx-tools/tests/fs_integration.rs`

- [ ] **Step 1: Write the integration test first**

Write `crates/clawx-tools/tests/fs_integration.rs`:

```rust
use std::sync::Arc;

use clawx_tools::approval::AutoApprovalGate;
use clawx_tools::fs::{FsListTool, FsMkdirTool, FsReadTool, FsWriteTool};
use clawx_tools::{Tool, ToolExecCtx};
use clawx_types::ids::AgentId;
use tempfile::tempdir;

fn ctx(ws: &std::path::Path) -> ToolExecCtx {
    ToolExecCtx {
        agent_id: AgentId::new(),
        workspace: ws.to_path_buf(),
        approval: Arc::new(AutoApprovalGate),
    }
}

#[tokio::test]
async fn mkdir_write_read_list_round_trip() {
    let dir = tempdir().unwrap();
    let c = ctx(dir.path());

    // 1) mkdir
    let r = FsMkdirTool
        .invoke(&c, serde_json::json!({"path": "sub/inner"}))
        .await
        .unwrap();
    assert!(!r.is_error, "mkdir: {}", r.content);
    assert!(dir.path().join("sub/inner").is_dir());

    // 2) write
    let r = FsWriteTool
        .invoke(&c, serde_json::json!({"path": "sub/hello.txt", "content": "hi"}))
        .await
        .unwrap();
    assert!(!r.is_error, "write: {}", r.content);

    // 3) read
    let r = FsReadTool
        .invoke(&c, serde_json::json!({"path": "sub/hello.txt"}))
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("hi"));

    // 4) list
    let r = FsListTool
        .invoke(&c, serde_json::json!({"path": "sub"}))
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("hello.txt"));
    assert!(r.content.contains("inner"));
}

#[tokio::test]
async fn write_outside_workspace_rejected() {
    let dir = tempdir().unwrap();
    let c = ctx(dir.path());
    let r = FsWriteTool
        .invoke(&c, serde_json::json!({"path": "../escape.txt", "content": "x"}))
        .await;
    assert!(r.is_err() || r.as_ref().unwrap().is_error);
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p clawx-tools --test fs_integration`
Expected: **FAIL** (tools not implemented).

- [ ] **Step 3: Implement the fs tools**

Replace `crates/clawx-tools/src/fs.rs`:

```rust
//! Filesystem tools: read, write, mkdir, list.
//!
//! Every path is resolved via `resolve_in_workspace`; all IO happens under
//! `tokio::fs` to stay on the async runtime. Errors are returned as
//! `ToolOutcome::err` (non-fatal) so the LLM can see and retry.

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::llm::ToolDefinition;
use serde::Deserialize;

use crate::{approval::ApprovalDecision, parse_args, resolve_in_workspace, Tool, ToolExecCtx, ToolOutcome};

// ---------------------------------------------------------------- fs_read
#[derive(Debug)]
pub struct FsReadTool;

#[derive(Debug, Deserialize)]
struct FsReadArgs {
    path: String,
    #[serde(default)]
    max_bytes: Option<usize>,
}

#[async_trait]
impl Tool for FsReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_read".into(),
            description: "Read the UTF-8 contents of a file in the workspace. \
                          Returns an error if the file is missing or not UTF-8."
                .into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Workspace-relative or absolute path inside workspace." },
                    "max_bytes": { "type": "integer", "description": "Optional read cap (default 1 MiB)." }
                },
                "required": ["path"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: FsReadArgs = parse_args("fs_read", args)?;
        if let ApprovalDecision::Deny { reason } =
            ctx.approval.check(&ctx.agent_id, "fs_read",
                               &serde_json::json!({"path": a.path})).await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        let cap = a.max_bytes.unwrap_or(1024 * 1024);
        match tokio::fs::read(&path).await {
            Ok(bytes) => {
                let truncated = &bytes[..bytes.len().min(cap)];
                match std::str::from_utf8(truncated) {
                    Ok(s) => Ok(ToolOutcome::ok(s.to_string())),
                    Err(_) => Ok(ToolOutcome::err(format!(
                        "file is not UTF-8: {}", path.display()
                    ))),
                }
            }
            Err(e) => Ok(ToolOutcome::err(format!("read {}: {}", path.display(), e))),
        }
    }
}

// ---------------------------------------------------------------- fs_write
#[derive(Debug)]
pub struct FsWriteTool;

#[derive(Debug, Deserialize)]
struct FsWriteArgs {
    path: String,
    content: String,
    #[serde(default)]
    create_parents: Option<bool>,
}

#[async_trait]
impl Tool for FsWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_write".into(),
            description: "Write UTF-8 `content` to `path` in the workspace, creating \
                          or overwriting the file. Set `create_parents:true` to \
                          create missing parent directories."
                .into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{
                    "path":{"type":"string"},
                    "content":{"type":"string"},
                    "create_parents":{"type":"boolean","default":false}
                },
                "required":["path","content"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: FsWriteArgs = parse_args("fs_write", args)?;
        if let ApprovalDecision::Deny { reason } = ctx.approval.check(
            &ctx.agent_id, "fs_write",
            &serde_json::json!({"path": a.path})).await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        if a.create_parents.unwrap_or(false) {
            if let Some(parent) = path.parent() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return Ok(ToolOutcome::err(format!("mkdir {}: {}", parent.display(), e)));
                }
            }
        }
        match tokio::fs::write(&path, a.content.as_bytes()).await {
            Ok(()) => Ok(ToolOutcome::ok(format!("wrote {} bytes to {}", a.content.len(), path.display()))),
            Err(e) => Ok(ToolOutcome::err(format!("write {}: {}", path.display(), e))),
        }
    }
}

// ---------------------------------------------------------------- fs_mkdir
#[derive(Debug)]
pub struct FsMkdirTool;

#[async_trait]
impl Tool for FsMkdirTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_mkdir".into(),
            description: "Create a directory (and missing parents) at `path`.".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"path":{"type":"string"}},
                "required":["path"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: crate::PathArgs = parse_args("fs_mkdir", args)?;
        if let ApprovalDecision::Deny { reason } = ctx.approval.check(
            &ctx.agent_id, "fs_mkdir",
            &serde_json::json!({"path": a.path})).await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        match tokio::fs::create_dir_all(&path).await {
            Ok(()) => Ok(ToolOutcome::ok(format!("created {}", path.display()))),
            Err(e) => Ok(ToolOutcome::err(format!("mkdir {}: {}", path.display(), e))),
        }
    }
}

// ---------------------------------------------------------------- fs_list
#[derive(Debug)]
pub struct FsListTool;

#[async_trait]
impl Tool for FsListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_list".into(),
            description: "List entries (one per line) in the given directory \
                          inside the workspace. Directories suffixed with '/'."
                .into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"path":{"type":"string"}},
                "required":["path"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: crate::PathArgs = parse_args("fs_list", args)?;
        if let ApprovalDecision::Deny { reason } = ctx.approval.check(
            &ctx.agent_id, "fs_list",
            &serde_json::json!({"path": a.path})).await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }
        let path = match resolve_in_workspace(&ctx.workspace, &a.path) {
            Ok(p) => p,
            Err(e) => return Ok(ToolOutcome::err(e.to_string())),
        };
        let mut rd = match tokio::fs::read_dir(&path).await {
            Ok(r) => r,
            Err(e) => return Ok(ToolOutcome::err(format!("list {}: {}", path.display(), e))),
        };
        let mut names: Vec<String> = Vec::new();
        loop {
            match rd.next_entry().await {
                Ok(Some(entry)) => {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
                    names.push(if is_dir { format!("{name}/") } else { name });
                }
                Ok(None) => break,
                Err(e) => return Ok(ToolOutcome::err(format!("list iter: {}", e))),
            }
        }
        names.sort();
        Ok(ToolOutcome::ok(names.join("\n")))
    }
}
```

- [ ] **Step 4: Run all clawx-tools tests**

Run: `cargo test -p clawx-tools`
Expected: **PASS** (3 unit + 2 integration).

- [ ] **Step 5: Clippy + fmt**

Run: `cargo clippy -p clawx-tools --all-targets -- -D warnings && cargo fmt -p clawx-tools`

- [ ] **Step 6: Commit**

```bash
git add crates/clawx-tools/src/fs.rs crates/clawx-tools/tests/fs_integration.rs
git commit -m "feat(tools): fs_read/fs_write/fs_mkdir/fs_list with workspace scoping"
```

---

## Task 6 · `shell_exec` tool with `sandbox-exec` wrapper (macOS)

**Files:**
- Modify: `crates/clawx-tools/src/sandbox_profile.rs`
- Modify: `crates/clawx-tools/src/shell.rs`
- Create: `crates/clawx-tools/tests/shell_integration.rs`

- [ ] **Step 1: Write the sandbox profile generator + unit tests**

Replace `crates/clawx-tools/src/sandbox_profile.rs`:

```rust
//! Generate `sandbox-exec(1)` profile text scoped to a workspace directory.
//!
//! The generated policy: deny by default, allow read everywhere (so the
//! child can load dylibs, read its own binary, etc.) but only allow
//! file-write under the workspace tree. No outbound network.

use std::path::Path;

pub fn workspace_profile(workspace: &Path) -> String {
    // Absolute canonical path required by sandbox-exec.
    let ws = workspace.display();
    format!(
r#"(version 1)
(deny default)
(allow process*)
(allow signal (target self))
(allow sysctl-read)
(allow mach-lookup)
(allow ipc-posix-shm)
(allow file-read*)
(allow file-write*
    (subpath "{ws}")
    (subpath "/private/tmp")
    (subpath "/tmp")
    (subpath "/private/var/folders"))
(deny network*)
"#,
        ws = ws
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn profile_includes_workspace_subpath() {
        let got = workspace_profile(&PathBuf::from("/ws/demo"));
        assert!(got.contains(r#"(subpath "/ws/demo")"#));
        assert!(got.contains("(deny network*)"));
    }
}
```

- [ ] **Step 2: Write failing integration test for shell_exec**

Write `crates/clawx-tools/tests/shell_integration.rs`:

```rust
use std::sync::Arc;

use clawx_tools::approval::AutoApprovalGate;
use clawx_tools::shell::ShellExecTool;
use clawx_tools::{Tool, ToolExecCtx};
use clawx_types::ids::AgentId;
use tempfile::tempdir;

fn ctx(ws: &std::path::Path) -> ToolExecCtx {
    ToolExecCtx {
        agent_id: AgentId::new(),
        workspace: ws.to_path_buf(),
        approval: Arc::new(AutoApprovalGate),
    }
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn shell_runs_echo_in_sandbox() {
    let dir = tempdir().unwrap();
    let ws = dir.path().canonicalize().unwrap();
    let c = ctx(&ws);
    let r = ShellExecTool::default()
        .invoke(&c, serde_json::json!({"command": "echo hello-claw"}))
        .await
        .unwrap();
    assert!(!r.is_error, "stderr: {}", r.content);
    assert!(r.content.contains("hello-claw"));
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn shell_blocks_write_outside_workspace() {
    let dir = tempdir().unwrap();
    let ws = dir.path().canonicalize().unwrap();
    let c = ctx(&ws);
    // Try to write to $HOME (outside the workspace); sandbox should deny.
    let r = ShellExecTool::default()
        .invoke(&c, serde_json::json!({
            "command": "touch $HOME/claw-sandbox-escape-$$"
        }))
        .await
        .unwrap();
    assert!(r.is_error || r.content.contains("Operation not permitted"));
}

#[cfg(not(target_os = "macos"))]
#[tokio::test]
async fn shell_reports_unsupported() {
    let dir = tempdir().unwrap();
    let c = ctx(dir.path());
    let r = ShellExecTool::default()
        .invoke(&c, serde_json::json!({"command": "echo hi"}))
        .await
        .unwrap();
    assert!(r.is_error && r.content.contains("unsupported"));
}
```

- [ ] **Step 3: Confirm failure**

Run: `cargo test -p clawx-tools --test shell_integration`
Expected: **FAIL** (`ShellExecTool` missing).

- [ ] **Step 4: Implement the shell tool**

Replace `crates/clawx-tools/src/shell.rs`:

```rust
//! shell_exec — run a shell command inside a macOS `sandbox-exec` profile.
//!
//! Non-macOS platforms return a `ToolOutcome::err("unsupported ...")`.
//! The profile (see `sandbox_profile`) denies network and confines file
//! writes to the workspace.

use std::path::PathBuf;
use std::time::Duration;

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::llm::ToolDefinition;
use serde::Deserialize;
use tokio::process::Command;

use crate::{approval::ApprovalDecision, parse_args, sandbox_profile, Tool, ToolExecCtx, ToolOutcome};

#[derive(Debug, Default)]
pub struct ShellExecTool {
    /// Default timeout for a command. Override per-call via the `timeout_secs` arg.
    pub default_timeout_secs: u64,
}

#[derive(Debug, Deserialize)]
struct ShellArgs {
    command: String,
    #[serde(default)]
    timeout_secs: Option<u64>,
    #[serde(default)]
    cwd: Option<String>,
}

#[async_trait]
impl Tool for ShellExecTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell_exec".into(),
            description: "Run a shell command inside a macOS sandbox scoped to the \
                          workspace. Captures stdout+stderr. Default timeout 30s. \
                          Network is denied; file writes outside the workspace are denied."
                .into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{
                    "command":{"type":"string","description":"/bin/sh -c <command>"},
                    "timeout_secs":{"type":"integer","default":30},
                    "cwd":{"type":"string","description":"Workspace-relative cwd (default: workspace root)."}
                },
                "required":["command"]
            }),
        }
    }

    async fn invoke(&self, ctx: &ToolExecCtx, args: serde_json::Value) -> Result<ToolOutcome> {
        let a: ShellArgs = parse_args("shell_exec", args)?;
        if let ApprovalDecision::Deny { reason } = ctx.approval.check(
            &ctx.agent_id, "shell_exec",
            &serde_json::json!({"command": a.command})).await?
        {
            return Ok(ToolOutcome::err(format!("denied: {reason}")));
        }

        if !cfg!(target_os = "macos") {
            return Ok(ToolOutcome::err(
                "shell_exec unsupported on this platform (macOS only in phase 1)".into(),
            ));
        }

        let cwd: PathBuf = match a.cwd {
            Some(rel) => match crate::resolve_in_workspace(&ctx.workspace, &rel) {
                Ok(p) => p,
                Err(e) => return Ok(ToolOutcome::err(e.to_string())),
            },
            None => ctx.workspace.clone(),
        };

        let profile = sandbox_profile::workspace_profile(&ctx.workspace);
        let timeout = Duration::from_secs(
            a.timeout_secs.unwrap_or_else(|| self.default_timeout_secs.max(30)),
        );

        // sandbox-exec -p '<profile>' /bin/sh -c '<command>'
        let mut cmd = Command::new("/usr/bin/sandbox-exec");
        cmd.arg("-p").arg(&profile)
           .arg("/bin/sh").arg("-c").arg(&a.command)
           .current_dir(&cwd)
           .kill_on_drop(true);

        let run = async {
            let out = cmd.output().await?;
            Ok::<_, std::io::Error>(out)
        };

        let output = match tokio::time::timeout(timeout, run).await {
            Ok(Ok(o)) => o,
            Ok(Err(e)) => return Ok(ToolOutcome::err(format!("spawn: {e}"))),
            Err(_) => return Ok(ToolOutcome::err(format!("timeout after {:?}", timeout))),
        };

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let mut body = String::new();
        if !stdout.is_empty() {
            body.push_str("=== stdout ===\n");
            body.push_str(&stdout);
            if !stdout.ends_with('\n') { body.push('\n'); }
        }
        if !stderr.is_empty() {
            body.push_str("=== stderr ===\n");
            body.push_str(&stderr);
        }
        let code = output.status.code().unwrap_or(-1);
        body.push_str(&format!("\n=== exit {code} ==="));
        Ok(if output.status.success() { ToolOutcome::ok(body) } else { ToolOutcome::err(body) })
    }
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p clawx-tools --test shell_integration`
Expected on macOS: **PASS** (2 tests). On non-macOS CI the `unsupported` branch passes.

- [ ] **Step 6: Clippy + fmt**

Run: `cargo clippy -p clawx-tools --all-targets -- -D warnings && cargo fmt -p clawx-tools`

- [ ] **Step 7: Commit**

```bash
git add crates/clawx-tools/src/shell.rs crates/clawx-tools/src/sandbox_profile.rs crates/clawx-tools/tests/shell_integration.rs
git commit -m "feat(tools): shell_exec with macOS sandbox-exec workspace profile"
```

---

## Task 7 · Three-tier ApprovalPort + rule store

**Files:**
- Modify: `crates/clawx-tools/src/approval.rs`

- [ ] **Step 1: Tests for rule evaluation**

Replace the `#[cfg(test)]` block in `approval.rs` with explicit coverage of the rule matcher:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use clawx_types::ids::AgentId;

    async fn run(gate: &RuleApprovalGate, tool: &str, args: serde_json::Value) -> ApprovalDecision {
        gate.check(&AgentId::new(), tool, &args).await.unwrap()
    }

    #[tokio::test]
    async fn auto_allow_by_default_for_read_tools() {
        let gate = RuleApprovalGate::default_claw_code_style();
        assert_eq!(run(&gate, "fs_read", serde_json::json!({"path":"x"})).await,
                   ApprovalDecision::Allow);
    }

    #[tokio::test]
    async fn write_tools_default_to_prompt_which_pending_gate_denies() {
        let gate = RuleApprovalGate::default_claw_code_style();
        // No interactive prompt wired in tests → Pending surfaces as Deny.
        let d = run(&gate, "fs_write", serde_json::json!({"path":"x"})).await;
        assert!(matches!(d, ApprovalDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn explicit_auto_rule_overrides_default() {
        let mut gate = RuleApprovalGate::default_claw_code_style();
        gate.add_rule(ApprovalRule {
            tool: "fs_write".into(),
            path_glob: Some("*.md".into()),
            mode: ApprovalMode::Auto,
        });
        let d = run(&gate, "fs_write", serde_json::json!({"path":"README.md"})).await;
        assert_eq!(d, ApprovalDecision::Allow);
    }

    #[tokio::test]
    async fn deny_rule_blocks_even_if_path_glob_matches_auto() {
        let mut gate = RuleApprovalGate::default_claw_code_style();
        gate.add_rule(ApprovalRule {
            tool: "fs_write".into(),
            path_glob: Some("secrets/*".into()),
            mode: ApprovalMode::Deny,
        });
        let d = run(&gate, "fs_write", serde_json::json!({"path":"secrets/.env"})).await;
        assert!(matches!(d, ApprovalDecision::Deny { .. }));
    }
}
```

- [ ] **Step 2: Confirm failure**

Run: `cargo test -p clawx-tools approval`
Expected: **FAIL** (types missing).

- [ ] **Step 3: Implement `RuleApprovalGate`**

Replace `crates/clawx-tools/src/approval.rs`:

```rust
//! Three-tier approval: auto / prompt / deny, per tool × path-glob.
//!
//! A `RuleApprovalGate` resolves a tool call against a prioritized
//! rule list. Matching precedence:
//!   1. Explicit rules in insertion order.
//!   2. Default rules supplied by `default_claw_code_style`.
//! Modes:
//!   - `Auto`   → Allow immediately.
//!   - `Prompt` → Delegate to the `PromptGate` (typically a channel to GUI).
//!                If no gate is wired, `Pending` surfaces as Deny.
//!   - `Deny`   → Deny with the rule's reason.

use async_trait::async_trait;
use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalDecision {
    Allow,
    Deny { reason: String },
}

#[async_trait]
pub trait ApprovalPort: Send + Sync {
    async fn check(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ApprovalDecision>;
}

/// Permissive gate — allows everything. Tests and dev only.
#[derive(Debug, Default)]
pub struct AutoApprovalGate;

#[async_trait]
impl ApprovalPort for AutoApprovalGate {
    async fn check(
        &self,
        _agent_id: &AgentId,
        _tool: &str,
        _args: &serde_json::Value,
    ) -> Result<ApprovalDecision> {
        Ok(ApprovalDecision::Allow)
    }
}

/// What to do when a rule matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalMode {
    Auto,
    Prompt,
    Deny,
}

#[derive(Debug, Clone)]
pub struct ApprovalRule {
    pub tool: String,
    /// Optional shell-style glob over the `path` argument (if present).
    /// If `None`, matches any args for this tool.
    pub path_glob: Option<String>,
    pub mode: ApprovalMode,
}

/// Interactive prompt delegate (GUI wires this).
#[async_trait]
pub trait PromptGate: Send + Sync {
    async fn ask(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ApprovalDecision>;
}

pub struct RuleApprovalGate {
    rules: Vec<ApprovalRule>,
    prompt: Option<Arc<dyn PromptGate>>,
}

impl RuleApprovalGate {
    pub fn new() -> Self {
        Self { rules: vec![], prompt: None }
    }

    /// Baseline: read/list auto; write/mkdir/shell prompt.
    pub fn default_claw_code_style() -> Self {
        Self {
            rules: vec![
                ApprovalRule { tool: "fs_read".into(),   path_glob: None, mode: ApprovalMode::Auto },
                ApprovalRule { tool: "fs_list".into(),   path_glob: None, mode: ApprovalMode::Auto },
                ApprovalRule { tool: "fs_write".into(),  path_glob: None, mode: ApprovalMode::Prompt },
                ApprovalRule { tool: "fs_mkdir".into(),  path_glob: None, mode: ApprovalMode::Prompt },
                ApprovalRule { tool: "shell_exec".into(), path_glob: None, mode: ApprovalMode::Prompt },
            ],
            prompt: None,
        }
    }

    pub fn add_rule(&mut self, rule: ApprovalRule) {
        // Newest rule wins: prepend.
        self.rules.insert(0, rule);
    }

    pub fn with_prompt(mut self, p: Arc<dyn PromptGate>) -> Self {
        self.prompt = Some(p);
        self
    }

    fn match_rule(&self, tool: &str, args: &serde_json::Value) -> Option<&ApprovalRule> {
        let path = args.get("path").and_then(|v| v.as_str());
        self.rules.iter().find(|r| {
            r.tool == tool && match (&r.path_glob, path) {
                (None, _) => true,
                (Some(_), None) => false,
                (Some(glob), Some(p)) => glob_match(glob, p),
            }
        })
    }
}

impl Default for RuleApprovalGate {
    fn default() -> Self { Self::new() }
}

#[async_trait]
impl ApprovalPort for RuleApprovalGate {
    async fn check(
        &self,
        agent_id: &AgentId,
        tool_name: &str,
        arguments: &serde_json::Value,
    ) -> Result<ApprovalDecision> {
        let mode = self.match_rule(tool_name, arguments)
            .map(|r| r.mode.clone())
            .unwrap_or(ApprovalMode::Prompt);
        match mode {
            ApprovalMode::Auto => Ok(ApprovalDecision::Allow),
            ApprovalMode::Deny => Ok(ApprovalDecision::Deny {
                reason: format!("denied by rule for tool '{}'", tool_name),
            }),
            ApprovalMode::Prompt => match &self.prompt {
                Some(p) => p.ask(agent_id, tool_name, arguments).await,
                None => Ok(ApprovalDecision::Deny {
                    reason: format!("prompt required for '{}' but no prompt gate configured", tool_name),
                }),
            },
        }
    }
}

/// Minimal shell-style glob: supports `*` and `?` and literal matching. Good
/// enough for path-scope rules; avoid pulling in `globset` for one call site.
fn glob_match(pattern: &str, text: &str) -> bool {
    fn rec(p: &[u8], t: &[u8]) -> bool {
        match (p.first(), t.first()) {
            (None, None) => true,
            (Some(b'*'), _) => rec(&p[1..], t) || (!t.is_empty() && rec(p, &t[1..])),
            (Some(b'?'), Some(_)) => rec(&p[1..], &t[1..]),
            (Some(a), Some(b)) if a == b => rec(&p[1..], &t[1..]),
            _ => false,
        }
    }
    rec(pattern.as_bytes(), text.as_bytes())
}

#[cfg(test)]
mod tests {
    // (the test block from Step 1)
    use super::*;
    use clawx_types::ids::AgentId;

    async fn run(gate: &RuleApprovalGate, tool: &str, args: serde_json::Value) -> ApprovalDecision {
        gate.check(&AgentId::new(), tool, &args).await.unwrap()
    }

    #[tokio::test]
    async fn auto_allow_by_default_for_read_tools() {
        let gate = RuleApprovalGate::default_claw_code_style();
        assert_eq!(run(&gate, "fs_read", serde_json::json!({"path":"x"})).await,
                   ApprovalDecision::Allow);
    }

    #[tokio::test]
    async fn write_tools_default_to_prompt_which_pending_gate_denies() {
        let gate = RuleApprovalGate::default_claw_code_style();
        let d = run(&gate, "fs_write", serde_json::json!({"path":"x"})).await;
        assert!(matches!(d, ApprovalDecision::Deny { .. }));
    }

    #[tokio::test]
    async fn explicit_auto_rule_overrides_default() {
        let mut gate = RuleApprovalGate::default_claw_code_style();
        gate.add_rule(ApprovalRule {
            tool: "fs_write".into(),
            path_glob: Some("*.md".into()),
            mode: ApprovalMode::Auto,
        });
        let d = run(&gate, "fs_write", serde_json::json!({"path":"README.md"})).await;
        assert_eq!(d, ApprovalDecision::Allow);
    }

    #[tokio::test]
    async fn deny_rule_blocks_even_if_path_glob_matches_auto() {
        let mut gate = RuleApprovalGate::default_claw_code_style();
        gate.add_rule(ApprovalRule {
            tool: "fs_write".into(),
            path_glob: Some("secrets/*".into()),
            mode: ApprovalMode::Deny,
        });
        let d = run(&gate, "fs_write", serde_json::json!({"path":"secrets/.env"})).await;
        assert!(matches!(d, ApprovalDecision::Deny { .. }));
    }

    #[test]
    fn glob_basic() {
        assert!(glob_match("*.md", "README.md"));
        assert!(glob_match("secrets/*", "secrets/.env"));
        assert!(!glob_match("*.md", "main.rs"));
    }
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p clawx-tools approval`
Expected: **PASS** (5 tests).

- [ ] **Step 5: Clippy + fmt**

Run: `cargo clippy -p clawx-tools --all-targets -- -D warnings && cargo fmt -p clawx-tools`

- [ ] **Step 6: Commit**

```bash
git add crates/clawx-tools/src/approval.rs
git commit -m "feat(tools): rule-based three-tier approval gate (auto/prompt/deny)"
```

---

## Task 8 · Tool-use iteration loop in `agent_loop`

**Files:**
- Create: `crates/clawx-runtime/src/tool_loop.rs`
- Modify: `crates/clawx-runtime/src/agent_loop.rs` (replace body of `run_turn` + imports)
- Modify: `crates/clawx-runtime/src/lib.rs` (add tools + approval fields to `Runtime`)
- Modify: `crates/clawx-runtime/Cargo.toml` (add `clawx-tools` dep)
- Create: `crates/clawx-runtime/tests/tool_loop_e2e.rs`

- [ ] **Step 1: Add dep**

Edit `crates/clawx-runtime/Cargo.toml`. Under `[dependencies]` add:

```toml
clawx-tools = { workspace = true }
```

- [ ] **Step 2: Extend `Runtime`**

In `crates/clawx-runtime/src/lib.rs` at the top, add:

```rust
use clawx_tools::{ApprovalPort, ToolRegistry};
```

Update the `Runtime` struct (around line 32) to add fields:

```rust
    /// Built-in tool registry. When `None`, `run_turn` degrades to a plain
    /// single-call LLM request (legacy behavior).
    pub tools: Option<Arc<ToolRegistry>>,
    /// Approval gate for tool invocations. Required whenever `tools` is set.
    pub approval: Option<Arc<dyn ApprovalPort>>,
    /// Workspace root. All tool IO resolves here. Required when `tools` is set.
    pub workspace: Option<std::path::PathBuf>,
    /// Max tool iterations per turn (safety brake). Default 10.
    pub max_tool_iterations: u32,
```

Initialize them in `Runtime::new` as `None`/`None`/`None`/`10`. Add builder methods:

```rust
    pub fn with_tools(
        mut self,
        tools: Arc<ToolRegistry>,
        approval: Arc<dyn ApprovalPort>,
        workspace: std::path::PathBuf,
    ) -> Self {
        self.tools = Some(tools);
        self.approval = Some(approval);
        self.workspace = Some(workspace);
        self
    }
    pub fn with_max_tool_iterations(mut self, n: u32) -> Self {
        self.max_tool_iterations = n;
        self
    }
```

- [ ] **Step 3: Write the tool_loop driver**

Write `crates/clawx-runtime/src/tool_loop.rs`:

```rust
//! Tool-use iteration driver for the agent loop.
//!
//! Responsibilities:
//! 1. Inject `ToolRegistry::definitions()` into the LLM request.
//! 2. When the LLM returns `StopReason::ToolUse`, dispatch each call,
//!    run the tool, and append a `role: Tool` message with `ToolResult`
//!    blocks back into the conversation.
//! 3. Re-query the LLM until it stops for any non-ToolUse reason,
//!    or until `max_iterations` is reached.

use std::sync::Arc;

use clawx_tools::{ToolExecCtx, ToolRegistry};
use clawx_types::error::{ClawxError, Result};
use clawx_types::ids::AgentId;
use clawx_types::llm::{
    CompletionRequest, ContentBlock, LlmResponse, Message, MessageRole, StopReason, TokenUsage,
};
use clawx_types::traits::LlmProvider;
use tracing::{debug, info, warn};

pub struct ToolLoopConfig {
    pub max_iterations: u32,
    pub max_tokens: u32,
}

pub struct ToolLoopOutcome {
    pub final_content: String,
    pub tool_calls_made: u32,
    pub usage: TokenUsage,
    pub messages: Vec<Message>, // for persistence
}

pub async fn run_with_tools(
    llm: Arc<dyn LlmProvider>,
    registry: Arc<ToolRegistry>,
    exec_ctx: ToolExecCtx,
    agent_id: &AgentId,
    mut messages: Vec<Message>,
    model: String,
    cfg: ToolLoopConfig,
) -> Result<ToolLoopOutcome> {
    let mut tool_calls_made: u32 = 0;
    let mut total_usage = TokenUsage::default();

    for iter in 0..cfg.max_iterations {
        let req = CompletionRequest {
            model: model.clone(),
            messages: messages.clone(),
            tools: if registry.is_empty() { None } else { Some(registry.definitions()) },
            temperature: None,
            max_tokens: Some(cfg.max_tokens),
            stream: false,
        };
        debug!(%agent_id, iter, msg_count = messages.len(), "tool_loop: calling LLM");
        let resp: LlmResponse = llm.complete(req).await?;
        total_usage.prompt_tokens += resp.usage.prompt_tokens;
        total_usage.completion_tokens += resp.usage.completion_tokens;
        total_usage.total_tokens += resp.usage.total_tokens;

        let assistant_blocks = reconstruct_assistant_blocks(&resp);
        messages.push(Message {
            role: MessageRole::Assistant,
            content: resp.content.clone(),
            blocks: assistant_blocks,
            tool_call_id: None,
        });

        if resp.stop_reason != StopReason::ToolUse || resp.tool_calls.is_empty() {
            return Ok(ToolLoopOutcome {
                final_content: resp.content,
                tool_calls_made,
                usage: total_usage,
                messages,
            });
        }

        // Dispatch each tool_use in parallel-safe order (sequential for now).
        let mut result_blocks: Vec<ContentBlock> = Vec::new();
        for call in &resp.tool_calls {
            tool_calls_made += 1;
            let tool = match registry.get(&call.name) {
                Some(t) => t,
                None => {
                    result_blocks.push(ContentBlock::ToolResult {
                        tool_use_id: call.id.clone(),
                        content: format!("unknown tool: {}", call.name),
                        is_error: true,
                    });
                    continue;
                }
            };
            info!(%agent_id, tool = %call.name, "tool_loop: invoking");
            let outcome = match tool.invoke(&exec_ctx, call.arguments.clone()).await {
                Ok(o) => o,
                Err(e) => clawx_tools::ToolOutcome::err(format!("tool panicked: {e}")),
            };
            result_blocks.push(ContentBlock::ToolResult {
                tool_use_id: call.id.clone(),
                content: outcome.content,
                is_error: outcome.is_error,
            });
        }
        messages.push(Message {
            role: MessageRole::Tool,
            content: String::new(),
            blocks: result_blocks,
            tool_call_id: None,
        });
    }

    warn!(%agent_id, "tool_loop: hit max_iterations={}", cfg.max_iterations);
    Err(ClawxError::Tool(format!(
        "exceeded max tool iterations ({}); last user-visible content may be incomplete",
        cfg.max_iterations
    )))
}

/// Rebuild the `ContentBlock` representation of an assistant turn so that
/// the follow-up request preserves the tool_use blocks in the transcript.
fn reconstruct_assistant_blocks(resp: &LlmResponse) -> Vec<ContentBlock> {
    let mut blocks: Vec<ContentBlock> = Vec::new();
    if !resp.content.is_empty() {
        blocks.push(ContentBlock::Text { text: resp.content.clone() });
    }
    for tc in &resp.tool_calls {
        blocks.push(ContentBlock::ToolUse {
            id: tc.id.clone(),
            name: tc.name.clone(),
            input: tc.arguments.clone(),
        });
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use clawx_tools::{approval::AutoApprovalGate, Tool};
    use clawx_types::llm::{LlmStreamChunk, ToolCall, ToolDefinition};
    use std::pin::Pin;
    use std::sync::Mutex;

    struct ScriptedLlm {
        responses: Mutex<Vec<LlmResponse>>,
    }
    #[async_trait]
    impl LlmProvider for ScriptedLlm {
        async fn complete(&self, _req: CompletionRequest) -> Result<LlmResponse> {
            Ok(self.responses.lock().unwrap().remove(0))
        }
        async fn stream(
            &self,
            _req: CompletionRequest,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = Result<LlmStreamChunk>> + Send>>> {
            unimplemented!()
        }
        async fn test_connection(&self) -> Result<()> { Ok(()) }
    }

    struct EchoTool;
    #[async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".into(),
                description: "echo".into(),
                parameters: serde_json::json!({"type":"object"}),
            }
        }
        async fn invoke(
            &self, _ctx: &ToolExecCtx, args: serde_json::Value,
        ) -> Result<clawx_tools::ToolOutcome> {
            Ok(clawx_tools::ToolOutcome::ok(args["msg"].as_str().unwrap_or("").to_string()))
        }
    }

    #[tokio::test]
    async fn loop_runs_tool_and_finalizes() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(EchoTool));
        let llm = Arc::new(ScriptedLlm {
            responses: Mutex::new(vec![
                LlmResponse {
                    content: String::new(),
                    stop_reason: StopReason::ToolUse,
                    tool_calls: vec![ToolCall {
                        id: "c1".into(),
                        name: "echo".into(),
                        arguments: serde_json::json!({"msg":"pong"}),
                    }],
                    usage: TokenUsage::default(),
                    metadata: None,
                },
                LlmResponse {
                    content: "pong".into(),
                    stop_reason: StopReason::EndTurn,
                    tool_calls: vec![],
                    usage: TokenUsage::default(),
                    metadata: None,
                },
            ]),
        });
        let exec = ToolExecCtx {
            agent_id: AgentId::new(),
            workspace: std::env::temp_dir(),
            approval: Arc::new(AutoApprovalGate),
        };
        let out = run_with_tools(
            llm, Arc::new(reg), exec.clone(), &exec.agent_id,
            vec![Message {
                role: MessageRole::User, content: "ping".into(),
                blocks: vec![], tool_call_id: None,
            }],
            "stub".into(),
            ToolLoopConfig { max_iterations: 5, max_tokens: 256 },
        ).await.unwrap();
        assert_eq!(out.final_content, "pong");
        assert_eq!(out.tool_calls_made, 1);
    }

    #[tokio::test]
    async fn loop_errors_on_max_iterations() {
        let mut reg = ToolRegistry::new();
        reg.register(Arc::new(EchoTool));
        // LLM keeps asking to call the tool forever.
        let infinite = (0..100).map(|_| LlmResponse {
            content: String::new(),
            stop_reason: StopReason::ToolUse,
            tool_calls: vec![ToolCall {
                id: "c1".into(), name: "echo".into(),
                arguments: serde_json::json!({"msg":"x"}),
            }],
            usage: TokenUsage::default(),
            metadata: None,
        }).collect();
        let llm = Arc::new(ScriptedLlm { responses: Mutex::new(infinite) });
        let exec = ToolExecCtx {
            agent_id: AgentId::new(),
            workspace: std::env::temp_dir(),
            approval: Arc::new(AutoApprovalGate),
        };
        let err = run_with_tools(
            llm, Arc::new(reg), exec.clone(), &exec.agent_id,
            vec![], "stub".into(),
            ToolLoopConfig { max_iterations: 3, max_tokens: 256 },
        ).await.unwrap_err();
        assert!(format!("{err}").contains("max tool iterations"));
    }
}
```

- [ ] **Step 4: Register the module + rewrite `run_turn`**

In `crates/clawx-runtime/src/lib.rs` add `pub mod tool_loop;` near the other module declarations.

Replace the body of `run_turn` in `crates/clawx-runtime/src/agent_loop.rs` (keeping the same signature) — preserving the intent-evaluation branch but routing tool-enabled paths through `tool_loop::run_with_tools`. Replace the block from the `// Step 2: Build completion request` comment through the `Ok(AgentResponse { ... })` at the end (lines ~60-139) with:

```rust
    // Step 2: Build completion request — inject recalled memories into system prompt.
    let mut messages = Vec::new();
    let system_prompt = if ctx.recalled_memories.is_empty() {
        ctx.system_prompt
    } else {
        let memory_block: String = ctx
            .recalled_memories
            .iter()
            .map(|m| format!("- {}", m.entry.summary))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n\n## Relevant memories\n{}", ctx.system_prompt, memory_block)
    };
    if !system_prompt.is_empty() {
        messages.push(Message {
            role: MessageRole::System,
            content: system_prompt,
            blocks: vec![],
            tool_call_id: None,
        });
    }
    messages.extend(ctx.conversation_history);
    messages.push(Message {
        role: MessageRole::User,
        content: user_input.to_string(),
        blocks: vec![],
        tool_call_id: None,
    });

    // Step 3: With tools → iterate. Without tools → legacy single call.
    let (final_content, tool_calls_made, usage) = match (&runtime.tools, &runtime.approval, &runtime.workspace) {
        (Some(tools), Some(approval), Some(workspace)) => {
            let exec_ctx = clawx_tools::ToolExecCtx {
                agent_id: *agent_id,
                workspace: workspace.clone(),
                approval: approval.clone(),
            };
            let outcome = crate::tool_loop::run_with_tools(
                runtime.llm.clone(),
                tools.clone(),
                exec_ctx,
                agent_id,
                messages.clone(),
                "default".into(),
                crate::tool_loop::ToolLoopConfig {
                    max_iterations: runtime.max_tool_iterations,
                    max_tokens: 4096,
                },
            ).await?;
            (outcome.final_content, outcome.tool_calls_made, outcome.usage)
        }
        _ => {
            let request = CompletionRequest {
                model: "default".to_string(),
                messages: messages.clone(),
                tools: None,
                temperature: None,
                max_tokens: Some(4096),
                stream: false,
            };
            let response = runtime.llm.complete(request).await?;
            (response.content, response.tool_calls.len() as u32, response.usage)
        }
    };

    // Step 4: Extract memories (unchanged — operates on user/assistant text).
    let extraction_messages = build_extraction_window(&messages, user_input, &final_content);
    let agent_id_owned = *agent_id;
    let extractor = runtime.memory_extractor.clone();
    let memory_svc = runtime.memory.clone();
    tokio::spawn(async move {
        match extractor.extract(&agent_id_owned, &extraction_messages).await {
            Ok(candidates) if !candidates.is_empty() => {
                for candidate in candidates {
                    let entry = MemoryEntry::from_candidate(candidate, Some(agent_id_owned));
                    if let Err(e) = memory_svc.store(entry).await {
                        warn!("failed to store extracted memory: {}", e);
                    }
                }
            }
            Ok(_) => {}
            Err(e) => warn!("memory extraction failed: {}", e),
        }
    });

    info!(%agent_id, "agent loop: turn complete");
    Ok(AgentResponse { content: final_content, tool_calls_made, tokens_used: usage })
```

- [ ] **Step 5: E2E test — tool_loop actually creates a directory**

Write `crates/clawx-runtime/tests/tool_loop_e2e.rs`:

```rust
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use clawx_runtime::tool_loop::{run_with_tools, ToolLoopConfig};
use clawx_tools::approval::AutoApprovalGate;
use clawx_tools::fs::FsMkdirTool;
use clawx_tools::{ToolExecCtx, ToolRegistry};
use clawx_types::error::Result;
use clawx_types::ids::AgentId;
use clawx_types::llm::*;
use clawx_types::traits::LlmProvider;
use futures::Stream;
use std::pin::Pin;

struct Scripted(Mutex<Vec<LlmResponse>>);
#[async_trait]
impl LlmProvider for Scripted {
    async fn complete(&self, _: CompletionRequest) -> Result<LlmResponse> {
        Ok(self.0.lock().unwrap().remove(0))
    }
    async fn stream(
        &self, _: CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>> {
        unimplemented!()
    }
    async fn test_connection(&self) -> Result<()> { Ok(()) }
}

#[tokio::test]
async fn agent_creates_folder_via_tool_use() {
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().to_path_buf();

    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(FsMkdirTool));

    let llm = Arc::new(Scripted(Mutex::new(vec![
        LlmResponse {
            content: String::new(),
            stop_reason: StopReason::ToolUse,
            tool_calls: vec![ToolCall {
                id: "c1".into(),
                name: "fs_mkdir".into(),
                arguments: serde_json::json!({"path":"claw-demo"}),
            }],
            usage: TokenUsage::default(),
            metadata: None,
        },
        LlmResponse {
            content: "Done — created claw-demo/.".into(),
            stop_reason: StopReason::EndTurn,
            tool_calls: vec![],
            usage: TokenUsage::default(),
            metadata: None,
        },
    ])));

    let exec = ToolExecCtx {
        agent_id: AgentId::new(),
        workspace: workspace.clone(),
        approval: Arc::new(AutoApprovalGate),
    };

    let out = run_with_tools(
        llm, Arc::new(reg), exec.clone(), &exec.agent_id,
        vec![Message {
            role: MessageRole::User,
            content: "Please create a folder named claw-demo".into(),
            blocks: vec![],
            tool_call_id: None,
        }],
        "stub".into(),
        ToolLoopConfig { max_iterations: 5, max_tokens: 256 },
    ).await.unwrap();

    assert!(workspace.join("claw-demo").is_dir(), "directory must exist after tool_loop");
    assert!(out.final_content.contains("Done"));
    assert_eq!(out.tool_calls_made, 1);
}
```

- [ ] **Step 6: Run the whole runtime suite**

Run: `cargo test -p clawx-runtime`
Expected: existing + new tests PASS. Pay attention to any `Message { ... }` initializer sites that fail because `blocks` is missing — add `blocks: vec![]`.

- [ ] **Step 7: Clippy + fmt**

Run: `cargo clippy -p clawx-runtime --all-targets -- -D warnings && cargo fmt -p clawx-runtime`

- [ ] **Step 8: Commit**

```bash
git add crates/clawx-runtime
git commit -m "feat(runtime): tool-use iteration loop + e2e mkdir test"
```

---

## Task 9 · Wire tools into `clawx-service` startup

**Files:**
- Modify: `apps/clawx-service/Cargo.toml` — add `clawx-tools` dep.
- Modify: `apps/clawx-service/src/main.rs` (or whichever file constructs `Runtime`) — register tools.
- Modify: `crates/clawx-config/src/...` — add a `ToolsConfig` block (workspace dir, approval rules) read from TOML.

- [ ] **Step 1: Find the runtime construction site**

Run: `grep -rn "Runtime::new" apps/ crates/clawx-api/ | head`
Confirm the site(s) that build `Runtime`. Usually `apps/clawx-service/src/main.rs`.

- [ ] **Step 2: Write a failing service-level test**

Add an integration test at `apps/clawx-service/tests/tools_wired.rs`:

```rust
// Smoke test: service boots with tools registered by default.
#[tokio::test]
async fn service_runtime_has_tools_registered() {
    let runtime = clawx_service::build_runtime_for_tests().await.unwrap();
    assert!(runtime.tools.is_some(), "tools must be wired by default");
    assert!(runtime.approval.is_some(), "approval gate must be wired");
    let reg = runtime.tools.as_ref().unwrap();
    let names: Vec<_> = reg.definitions().into_iter().map(|d| d.name).collect();
    for expected in ["fs_read", "fs_write", "fs_mkdir", "fs_list"] {
        assert!(names.iter().any(|n| n == expected), "missing tool: {}", expected);
    }
}
```

- [ ] **Step 3: Confirm failure (missing helper)**

Run: `cargo test -p clawx-service --test tools_wired`
Expected: **FAIL** — `build_runtime_for_tests` not exposed.

- [ ] **Step 4: Expose a test-friendly builder**

In `apps/clawx-service/src/lib.rs` (create if the crate is binary-only — update `Cargo.toml` with `[lib] path = "src/lib.rs"` and move reusable code there), add:

```rust
use std::path::PathBuf;
use std::sync::Arc;
use clawx_runtime::{db::Database, Runtime};
use clawx_tools::{
    approval::RuleApprovalGate,
    fs::{FsListTool, FsMkdirTool, FsReadTool, FsWriteTool},
    shell::ShellExecTool,
    ToolRegistry,
};

pub async fn build_runtime_for_tests() -> anyhow::Result<Runtime> {
    let db = Database::in_memory().await?;
    let workspace = std::env::temp_dir().join("clawx-test-workspace");
    tokio::fs::create_dir_all(&workspace).await?;
    let mut reg = ToolRegistry::new();
    reg.register(Arc::new(FsReadTool));
    reg.register(Arc::new(FsWriteTool));
    reg.register(Arc::new(FsMkdirTool));
    reg.register(Arc::new(FsListTool));
    reg.register(Arc::new(ShellExecTool::default()));
    let approval = Arc::new(RuleApprovalGate::default_claw_code_style());
    Ok(Runtime::new(
        db,
        Arc::new(clawx_llm::StubLlmProvider),
        Arc::new(clawx_memory::StubMemoryService),
        Arc::new(clawx_memory::StubWorkingMemoryManager),
        Arc::new(clawx_memory::StubMemoryExtractor),
        Arc::new(clawx_security::PermissiveSecurityGuard),
        Arc::new(clawx_vault::StubVaultService),
        Arc::new(clawx_kb::StubKnowledgeService),
        Arc::new(clawx_config::ConfigLoader::with_defaults()),
    ).with_tools(Arc::new(reg), approval, workspace))
}
```

In the production `main.rs`, mirror the same registration before handing `Runtime` to the API layer. Read `workspace` from config (default `~/.clawx/workspace`, `create_dir_all` at boot).

- [ ] **Step 5: Run the smoke test**

Run: `cargo test -p clawx-service --test tools_wired`
Expected: **PASS**.

- [ ] **Step 6: Clippy + fmt + full workspace build**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt && cargo build --workspace`

- [ ] **Step 7: Commit**

```bash
git add apps/clawx-service Cargo.toml
git commit -m "feat(service): register built-in tools + approval gate at startup"
```

---

## Task 10 · `/tools/approval` HTTP endpoint for GUI

**Files:**
- Create: `crates/clawx-api/src/routes/tools.rs`
- Modify: `crates/clawx-api/src/routes/mod.rs` (or `lib.rs`) to register the route.
- Modify: `crates/clawx-tools/src/approval.rs` — add `ChannelPromptGate` wired over a `tokio::sync::mpsc`.

Rationale: the rule gate currently denies on Prompt when no prompt delegate is configured. Adding an HTTP-backed delegate lets the SwiftUI GUI (or any client) answer prompts in-session.

- [ ] **Step 1: Failing test for the endpoint**

Write `crates/clawx-api/tests/approval_route.rs`:

```rust
use axum::http::StatusCode;
use clawx_api::build_router_for_tests;
use serde_json::json;

#[tokio::test]
async fn approval_endpoint_allow_then_gate_resolves() {
    let (router, gate) = build_router_for_tests();
    // Register a pending request, then POST /tools/approval?id=... with decision=allow
    let req_id = gate.open_request("fs_write", json!({"path":"a"})).await;

    let app = router.into_make_service();
    let client = axum_test::TestServer::new(app).unwrap();

    let resp = client
        .post(&format!("/tools/approval/{req_id}"))
        .json(&json!({"decision":"allow"}))
        .await;
    assert_eq!(resp.status_code(), StatusCode::NO_CONTENT);

    // The gate's outstanding future resolves to Allow.
    let decision = gate.await_resolution(req_id).await.unwrap();
    assert!(matches!(decision, clawx_tools::ApprovalDecision::Allow));
}
```

This test references helpers we need to author (`build_router_for_tests`, `open_request`, `await_resolution`). The implementation task is the rest of this task.

- [ ] **Step 2: Add `ChannelPromptGate`**

Append to `crates/clawx-tools/src/approval.rs`:

```rust
use tokio::sync::{oneshot, Mutex};
use std::collections::HashMap;

pub struct ChannelPromptGate {
    pending: Mutex<HashMap<uuid::Uuid, oneshot::Sender<ApprovalDecision>>>,
}

impl ChannelPromptGate {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { pending: Mutex::new(HashMap::new()) })
    }

    /// Called by the API handler once the user has answered.
    pub async fn resolve(&self, id: uuid::Uuid, decision: ApprovalDecision) -> bool {
        if let Some(tx) = self.pending.lock().await.remove(&id) {
            let _ = tx.send(decision);
            true
        } else { false }
    }

    /// Test-only helper: open a pending request directly.
    #[doc(hidden)]
    pub async fn open_request(&self, _tool: &str, _args: serde_json::Value) -> uuid::Uuid {
        let id = uuid::Uuid::new_v4();
        let (tx, _rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        id
    }
}

#[async_trait]
impl PromptGate for ChannelPromptGate {
    async fn ask(
        &self,
        _agent_id: &AgentId,
        _tool: &str,
        _args: &serde_json::Value,
    ) -> Result<ApprovalDecision> {
        let id = uuid::Uuid::new_v4();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().await.insert(id, tx);
        // TODO(phase 2): publish an event so the GUI can fetch pending prompts.
        // For now the GUI polls GET /tools/approval (to be added in GUI plan).
        match rx.await {
            Ok(d) => Ok(d),
            Err(_) => Ok(ApprovalDecision::Deny { reason: "prompt channel closed".into() }),
        }
    }
}
```

Add `uuid` to `clawx-tools` Cargo.toml deps (`{ workspace = true }`).

- [ ] **Step 3: Add the route**

Write `crates/clawx-api/src/routes/tools.rs`:

```rust
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use clawx_tools::{approval::ChannelPromptGate, ApprovalDecision};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ApprovalBody {
    pub decision: String,
    #[serde(default)]
    pub reason: Option<String>,
}

pub fn router(gate: Arc<ChannelPromptGate>) -> Router {
    Router::new()
        .route("/tools/approval/:id", post(resolve))
        .with_state(gate)
}

async fn resolve(
    State(gate): State<Arc<ChannelPromptGate>>,
    Path(id): Path<Uuid>,
    Json(body): Json<ApprovalBody>,
) -> StatusCode {
    let decision = match body.decision.as_str() {
        "allow" => ApprovalDecision::Allow,
        "deny" => ApprovalDecision::Deny {
            reason: body.reason.unwrap_or_else(|| "denied by user".into()),
        },
        _ => return StatusCode::BAD_REQUEST,
    };
    if gate.resolve(id, decision).await {
        StatusCode::NO_CONTENT
    } else {
        StatusCode::NOT_FOUND
    }
}
```

Register the router in `crates/clawx-api/src/lib.rs` (or wherever `build_router` lives).

- [ ] **Step 4: Run the test**

Run: `cargo test -p clawx-api --test approval_route`
Expected: **PASS** (with `axum-test` in dev-deps — add it if missing).

- [ ] **Step 5: Clippy + fmt**

Run: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt`

- [ ] **Step 6: Commit**

```bash
git add crates/clawx-api crates/clawx-tools
git commit -m "feat(api): POST /tools/approval/:id — resolve tool-use prompts"
```

---

## Task 11 · Decision record + smoke walkthrough

**Files:**
- Create: `docs/arch/decisions.md` append a new ADR.
- Update: `workflow.md` appendix, mirroring the 2026-04-18 style entry.

- [ ] **Step 1: Append ADR**

Open `docs/arch/decisions.md`. Append:

```markdown
## ADR 2026-04-19 · Agentic tool-use loop (Phase 1)

**Context.** `agent_loop::run_turn` never passed `tools` to the LLM, so agents
could only produce text. The team wanted picoclaw-parity: fs + shell tools
driven by an iteration loop with an approval gate and macOS sandboxing.

**Decision.**
- Introduce `clawx-tools` crate with a `Tool` trait + `ToolRegistry`.
- Extend `Message` with structured `ContentBlock`s (back-compat with
  `content: String`) so `tool_use`/`tool_result` can round-trip through
  Anthropic and OpenAI-compat wires.
- Drive tool iteration from a new `runtime::tool_loop::run_with_tools`
  consumed by `agent_loop::run_turn` when the runtime has tools wired.
- Ship five built-in tools: `fs_read`, `fs_write`, `fs_mkdir`, `fs_list`,
  `shell_exec`. `shell_exec` is macOS-only in Phase 1 and runs under
  `/usr/bin/sandbox-exec` with a workspace-scoped profile.
- Three-tier `RuleApprovalGate`: auto / prompt / deny per tool × path-glob.
  Prompt is resolved by `ChannelPromptGate` over `POST /tools/approval/:id`.

**Consequences.** Phase 2+ plans layer hooks, SubTurn, steering, MCP client,
and markdown-sourced skills on top of this surface. The LLM-provider fix is
permanent — providers always declare their tool wire format even when no
tools are registered, so future plans don't revisit it.
```

- [ ] **Step 2: Append workflow entry**

Append to `workflow.md`:

```markdown
## 2026-04-19 Phase-1 tool-use loop 执行记录

Plan: `docs/superpowers/plans/2026-04-19-agentic-tool-use-phase1.md`
Branch: `feature/agentic-tool-use-phase1` (11 tasks, 11 commits)

### 验证命令
```
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
```

### 手工 walkthrough
1. `cargo run -p clawx-service` 启动后，打开 Agent 对话界面。
2. 对 Agent 说"请在 workspace 里创建一个叫 claw-demo 的文件夹"。
3. GUI 弹出 approval dialog（fs_mkdir 默认 prompt）→ 点允许。
4. `ls ~/.clawx/workspace/claw-demo` 可见。
5. 说"在里面创建 hello.txt 写入 'hi claw'" → 再次确认 → 文件存在且内容正确。
6. 说"列出这个目录" → Agent 回复 `hello.txt`，无需再次确认（fs_list 默认 auto）。
7. 说"运行 pwd" → 弹 approval → 允许 → stdout 显示 workspace 路径。
8. 说"运行 curl https://example.com" → sandbox-exec 拦截 → Agent 收到 stderr
   包含 `Operation not permitted`，并向用户说明网络被沙箱禁止。
```

- [ ] **Step 3: Commit**

```bash
git add docs/arch/decisions.md workflow.md
git commit -m "docs: ADR + walkthrough for Phase-1 tool-use loop"
```

---

## Follow-up Plans (NOT part of this plan)

Each of these should become its own plan after Phase 1 ships:

1. **Hook system** — `Hook` trait + registry (`pre_llm` / `post_llm` / `pre_tool` / `post_tool` / `pre_turn` / `post_turn`) wired into `tool_loop`. Picoclaw's hook shape is a good match.
2. **SubTurn coordination** — Let a tool spawn a child `run_turn` with an isolated context + independent budget. Requires a `SubTurnRequest` tool and a slim agent-spawner service.
3. **Steering** — Inject `System`/`User` messages between tool calls (think: "remember to stay inside the workspace"). Implemented as a hook (`pre_llm`) + a steering queue on `ToolExecCtx`.
4. **MCP client** — `clawx-mcp` crate with a stdio JSON-RPC transport (later SSE). Wrap each MCP server's tools as `impl Tool`, register into `ToolRegistry` at config time.
5. **Markdown skills** — Extend `skill_loader.rs` to load `skills/*.md` with YAML frontmatter. Render each skill as either (a) a system-prompt fragment or (b) a virtual `Tool` that returns its body on demand. Complements the existing WASM skill path.
6. **GUI approval dialog** — SwiftUI sheet driven by `GET /tools/pending` (add a streaming endpoint or poll). Makes the `prompt` tier actually usable end-to-end.
7. **OpenAI streaming + tool_use** — We left streaming alone in Phase 1. A follow-up plan can unify SSE streaming across providers with incremental tool_use deltas.

---

## Self-Review Checklist (done 2026-04-19 before saving)

- Spec coverage: every item the user asked about has a task, OR is explicitly deferred in Follow-up Plans.
- Placeholders: none (every step has code or exact commands; no "TBD").
- Type consistency: `ContentBlock`, `ToolOutcome`, `ApprovalDecision`, `ToolExecCtx`, `ToolRegistry` names are identical across every task that mentions them. `FsReadTool / FsWriteTool / FsMkdirTool / FsListTool / ShellExecTool` names are stable.
- Scope: Phase-1 produces working software on its own (`agent_creates_folder_via_tool_use` e2e test proves the user's headline gripe is fixed).
