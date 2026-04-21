# Hermes Internal Surface (as depended upon by `hermes_bridge`)

Snapshot taken at `hermes-agent==0.10.0` (git SHA `7fc1e91811b7f5ceab0bdc85e01ca4f77a8555a5`, upstream `main` tip on 2026-04-22).

Updated whenever `backend/pyproject.toml` moves the pinned SHA. Every symbol below is one we **must** re-verify on any upstream bump.

## Home & paths

- `hermes_constants.get_hermes_home() -> Path` — reads env `HERMES_HOME`; falls back to `~/.hermes`.
- `hermes_constants.get_config_path() -> Path` — `~/.hermes/config.yaml`.
- `hermes_constants.get_skills_dir() -> Path` — `~/.hermes/skills/`.
- `hermes_constants.get_env_path() -> Path` — `~/.hermes/.env`.

## Turn loop

- Entrypoint: **`run_agent.AIAgent`** (class at `run_agent.py:690`).
- Construct with kwargs: `model`, `api_key`, `base_url`, `enabled_toolsets`, `session_id`, plus many callbacks (`stream_delta_callback`, `interim_assistant_callback`, `step_callback`, `tool_start_callback`, `tool_complete_callback`, `tool_progress_callback`, `thinking_callback`, `reasoning_callback`, `status_callback`).
- Single-turn call: **`AIAgent.chat(message: str, stream_callback=None) -> str`** (`run_agent.py:11871`). Returns the final assistant text directly.
- Full-turn call: **`AIAgent.run_conversation(user_message, system_message=None, conversation_history=None, task_id=None, stream_callback=None, persist_user_message=None) -> Dict[str, Any]`** (`run_agent.py:8582`). Returns `{"final_response": str, ... + message history}`.
- Both are **synchronous (blocking)**. Wrap in `anyio.to_thread.run_sync(...)` from the async FastAPI context.
- To get intermediate "thought" frames we use the callbacks set on `__init__`:
  - `interim_assistant_callback(text)` — each interim assistant step (what becomes `payload.thought=true` frames).
  - `tool_start_callback`, `tool_complete_callback` — tool activity.
  - The final assistant message is the return value of `run_conversation`.
- `main()` in `run_agent.py:11886` is the CLI entrypoint; we don't call it.

## Session store

- **`hermes_state.SessionDB`** (class at `hermes_state.py:115`).
- DB path: `get_hermes_home() / "state.db"` (constant `DEFAULT_DB_PATH` at `hermes_state.py:32`). **Not `sessions.db`** (the plan's initial guess was wrong — update `session_store.py` accordingly in Task 4.1.)
- Relevant public methods:
  - `create_session(...)` — we don't call this directly; AIAgent does.
  - `get_session(session_id) -> Optional[Dict]` — single session metadata.
  - `list_sessions_rich(...)` — paginated list with token counts, titles, costs. **Use this for `/api/sessions`.**
  - `get_messages(session_id) -> List[Dict]` — all messages for a session (role, content, tool_call_id, tool_calls, timestamp, etc.).
  - `get_messages_as_conversation(session_id) -> List[Dict]` — messages in the shape AIAgent expects when resuming.
  - `set_session_title(session_id, title) -> bool`
  - `get_session_title(session_id) -> Optional[str]`
  - `search_sessions(query)` — FTS5 over sessions.
  - `search_messages(query, session_id=None)` — FTS5 over messages.
  - `session_count(source=None) -> int`
  - `message_count(session_id=None) -> int`
  - `delete_session(session_id) -> bool` — for `DELETE /api/sessions/:id`.
  - `clear_messages(session_id)` — clear messages only, preserve session row.
  - `export_session(session_id) -> Optional[Dict]` — full export.

### Schema snapshot (for raw-SQL fallback only; prefer SessionDB methods)

```sql
CREATE TABLE sessions (
    id TEXT PRIMARY KEY,
    source TEXT NOT NULL,
    user_id TEXT,
    model TEXT,
    model_config TEXT,
    system_prompt TEXT,
    parent_session_id TEXT,
    started_at REAL NOT NULL,   -- unix epoch SECONDS (not millis)
    ended_at REAL,
    end_reason TEXT,
    message_count INTEGER DEFAULT 0,
    tool_call_count INTEGER DEFAULT 0,
    input_tokens INTEGER DEFAULT 0,
    output_tokens INTEGER DEFAULT 0,
    -- ... cache/reasoning token counts, cost columns ...
    title TEXT
);
CREATE TABLE messages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    session_id TEXT NOT NULL REFERENCES sessions(id),
    role TEXT NOT NULL,                -- "user" | "assistant" | "system" | "tool"
    content TEXT,
    tool_call_id TEXT,
    tool_calls TEXT,                   -- JSON
    tool_name TEXT,
    timestamp REAL NOT NULL,           -- unix epoch SECONDS
    token_count INTEGER,
    finish_reason TEXT,
    reasoning TEXT,
    reasoning_details TEXT,
    codex_reasoning_items TEXT
);
```

### Wire format translation

Our API exposes `created`/`updated` in **unix millis** (`apps/clawx-gui/src/lib/hermes-rest.ts` says so). Map: `milliseconds = int(started_at * 1000)` (or use `ended_at` when available as `updated`; else reuse `started_at`).

Gateway-specific `gateway.session.SessionStore` (at `gateway/session.py:550`) wraps `SessionDB` plus tracks messaging-platform session keys. **We do NOT use gateway.session.SessionStore — it is coupled to channel delivery (Telegram etc).** Our bridge only needs the SQL-level `hermes_state.SessionDB`.

## Toolset state

- **`toolsets.py`** top-level module. Public functions:
  - `get_all_toolsets() -> Dict[str, Dict[str, Any]]` — name → definition for every toolset (built-in + plugin + custom).
  - `get_toolset_names() -> List[str]`
  - `get_toolset(name) -> Optional[Dict]`
  - `resolve_toolset(name, visited=None) -> List[str]` — flatten composed toolsets to tool names.
  - `resolve_multiple_toolsets(names) -> List[str]`
  - `validate_toolset(name) -> bool`
  - `create_custom_toolset(...)` — build a new toolset.
  - `get_toolset_info(name) -> Dict[str, Any]`
- **Enable/disable persistence**: there is no single `enable(name)` / `disable(name)` function in `toolsets.py`. Enablement is a **per-AIAgent-construction** argument (`enabled_toolsets=` / `disabled_toolsets=` in `AIAgent.__init__`). For `PUT /api/tools/:name/state` we therefore maintain our own persistence at `~/.hermes/hermes_bridge_tools.json` and apply it on next `AIAgent` construction. Any mismatch with hermes's own configuration layer needs revisiting if hermes adds a programmatic toggle.
- **"blocked" status**: a toolset referencing a plugin/tool that cannot load (`ImportError`, missing API key, missing MCP server) should appear as `blocked`. Derive by inspecting `get_toolset(name)` and attempting lazy resolution; if it raises or the underlying `tools/` module flags a missing dep, mark blocked + populate `reason_code`.

## Skills

- Directory: `hermes_constants.get_skills_dir()` → `~/.hermes/skills/`.
- **Discovery**: `agent.skill_commands.scan_skill_commands() -> Dict[str, Dict[str, Any]]` (`skill_commands.py:338`) — returns every skill command indexed by name; the dict value has fields including the skill's frontmatter. Cached via `get_skill_commands()`.
- **Low-level helpers in `agent.skill_utils`**:
  - `parse_frontmatter(content) -> Tuple[Dict, str]` — YAML frontmatter + body.
  - `extract_skill_description(frontmatter) -> str`.
  - `iter_skill_index_files(skills_dir, filename)` — walks `SKILL.md` files.
  - `get_all_skills_dirs() -> List[Path]` — all searched dirs (user + plugin + external).
  - `get_disabled_skill_names(platform=None) -> Set[str]`.
- **Listing for `/api/skills`**: call `scan_skill_commands()` and project each entry to `{name, description}`. Use `extract_skill_description(...)` on the frontmatter dict if the direct field is absent.
- **Install**: hermes does not ship a programmatic installer. Not implemented in Phase 5; `SkillService.install` raises `NotImplementedError` and the REST handler returns 501. Follow-up work: either use hermes's ClawHub fetcher if/when it lands in `hermes_cli/`, or shell out to `hermes skills add …` and capture stdout.
- **Uninstall**: `shutil.rmtree(skill_dir)` on the matching sub-directory of `~/.hermes/skills/`. Do not touch directories outside that root.

## Upgrade brittleness

Every symbol above is one we must re-verify on any hermes-agent SHA bump. If upstream renames any of these, touch ONLY:

- `hermes_bridge/bridge/hermes_runner.py` (the AIAgent wrapper)
- `hermes_bridge/bridge/hermes_factory.py` (only file importing hermes internals)
- `hermes_bridge/bridge/session_store.py` (wraps SessionDB)
- `hermes_bridge/bridge/skill_service.py` (wraps scan_skill_commands + filesystem)
- `hermes_bridge/bridge/tool_service.py` (wraps get_all_toolsets + local state)

Nothing else in the adapter should need to change.
