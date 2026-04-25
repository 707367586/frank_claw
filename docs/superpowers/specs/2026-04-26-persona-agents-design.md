# Persona Agents — Design

Date: 2026-04-26
Status: Approved (brainstorming complete)

## 1. Goals

Wire the existing `clawx-gui` frontend to the running `hermes_bridge` backend so users can:

1. Have real LLM conversations through hermes-agent (already plumbed; needs verification end-to-end).
2. Maintain multiple **persona agents**. Each agent has its own name, look, system prompt, optional model override, toolset whitelist, persistent chat thread, and isolated filesystem workspace.
3. Create new agents from the UI; delete unwanted ones; switch between them; reset a single agent's conversation ("new conversation").

The mock 4-agent sidebar shipped in the homepage iteration is replaced by real, persistent data backed by `~/.hermes/agents.json`.

## 2. Non-Goals (v1)

- Editing an existing agent (delete + recreate is the workflow).
- Drag-to-reorder agents.
- Multiple chat sessions per agent (one persistent thread per agent).
- Concurrent turns across agents (process-wide `TERMINAL_CWD` is racy; v1 assumes the user only chats with one agent at a time).
- Garbage-collecting workspace directories on delete (the JSON entry is removed; the directory under `~/.hermes/workspaces/<id>/` stays so user files aren't lost).
- Uploading custom avatar images (12 fixed colors × 12 fixed lucide icons).
- Per-agent skill (hermes "skills") whitelist.
- Per-agent provider override (only `model` can be overridden; provider stays global).
- Polling each non-active agent for live status; non-active agents always show `Idle`.

## 3. Data Model

### 3.1 Agent record

```ts
type Agent = {
  id: string;                  // uuid v4, stable
  name: string;                // required
  description: string;         // short subtitle, may be ""
  color: string;               // hex like "#5749F4"
  icon: string;                // lucide icon name, e.g. "Code2"
  system_prompt: string;       // required, injected as ephemeral_system_prompt
  model: string | null;        // null = follow ~/.hermes/config.yaml
  enabled_toolsets: string[];  // [] = no whitelist (defer to hermes-agent defaults); non-empty = whitelist passed to AIAgent's enabled_toolsets
  workspace_dir: string;       // absolute path; default "~/.hermes/workspaces/<id>"
  current_session_id: string;  // uuid; matches a hermes-agent SQLite session
  created_at: number;          // unix millis
};
```

### 3.2 Storage

`~/.hermes/agents.json`:

```json
{
  "version": 1,
  "agents": [ /* Agent, ... */ ]
}
```

- Single-file atomic write: write to `agents.json.tmp`, then `os.replace` → `agents.json`.
- File missing on first read → seed 4 default agents (§3.3) and write.
- `version: 1` reserved for future migrations.

### 3.3 Default seeds (written on first read)

| id | name | color | icon | enabled_toolsets |
|---|---|---|---|---|
| `code` | 编程助手 | `#5749F4` | `Code2` | `["terminal","file","skills","debugging","code_execution"]` |
| `research` | 研究助手 | `#3B82F6` | `Search` | `["web","search","vision","session_search"]` |
| `writing` | 写作助手 | `#EC4899` | `PenTool` | `["file","memory","todo"]` |
| `data` | 数据分析 | `#F59E0B` | `BarChart3` | `["code_execution","file"]` |

Each seed gets a freshly generated `current_session_id`, `workspace_dir` set to `~/.hermes/workspaces/<id>/` (the directory itself is **not** created at seed time — `mkdir -p` runs lazily on the first turn from §4.5), `model: null`, and a Chinese `system_prompt` describing that role's responsibilities and tone (the actual prompt text is filled in during implementation; ~5–10 sentences each).

## 4. Backend

### 4.1 New module `hermes_bridge/bridge/agent_store.py`

`AgentStore(settings)`:
- `list() -> list[Agent]`
- `get(id) -> Agent | None`
- `create(payload: AgentCreate) -> Agent` — generates `id` (uuid4), `current_session_id` (uuid4), `created_at`, defaults `workspace_dir = settings.hermes_home / "workspaces" / id` when `payload.workspace_dir` is None, and `mkdir -p`'s the chosen directory eagerly so it exists before the first WS connect.
- `delete(id) -> None` — removes from JSON; **does not** delete the workspace directory.
- `rotate_session(id) -> str` — assigns a new `current_session_id`, persists, returns it.
- `_seed_if_missing()` — writes 4 default seeds when the file does not exist.
- Internal `_save(data)` writes atomically (`tmp + os.replace`).

`AgentCreate` is the same shape as `Agent` minus the server-controlled fields (`id`, `current_session_id`, `created_at`). `workspace_dir` is optional in the payload; when omitted the store assigns the default.

### 4.2 New endpoint `hermes_bridge/api/agents.py`

All routes bearer-protected via the existing `require_bearer_token` dependency.

| Method | Path | Body | Response |
|---|---|---|---|
| GET | `/api/agents` |  | `Agent[]` |
| POST | `/api/agents` | `AgentCreate` | `201 Agent` |
| DELETE | `/api/agents/{id}` |  | `204` (404 if unknown) |
| POST | `/api/agents/{id}/sessions` |  | `200 { session_id }` (404 if unknown) |

Validation (Pydantic): `name` non-empty, `system_prompt` non-empty, `color` matches `^#[0-9A-Fa-f]{6}$`, `icon` non-empty, `enabled_toolsets` is a string array (no membership check — unknown toolset names are simply ignored by hermes-agent).

### 4.3 New endpoint `hermes_bridge/api/toolsets.py`

`GET /api/toolsets` → `Toolset[]` where:

```ts
type Toolset = { name: string; description: string; tools: string[] };
```

Implementation: lazy `from toolsets import TOOLSETS` (matches the existing lazy-import pattern in `hermes_factory.py`), iterate the dict, project each entry to `{name, description, tools}`. If the import fails (broken env), return `[]` with a 200 — the frontend treats empty list as "no whitelist UI".

### 4.4 WS upgrade `hermes_bridge/ws/chat.py`

- Endpoint signature gains an optional query param: `agent_id: str | None = Query(default=None)`.
- `make_runner` factory signature changes to `Callable[[str, str | None], HermesRunner]` — i.e. `(session_id, agent_id) -> HermesRunner`.
- `bind_runner_factory(factory)` re-typed accordingly.
- Existing tests that monkeypatch `make_runner` need to accept the new second arg.
- **Backward compatibility**: connections that omit `agent_id` continue to work and use the global config.

### 4.5 `hermes_bridge/bridge/hermes_factory.py` — persona injection

`_HermesAgentAdapter.__init__(settings, session_id, agent: Agent | None = None)`:

- If `agent is None` → existing behavior unchanged.
- If `agent` given:
  - `model = agent.model or <global model from config.yaml>`
  - Pass `ephemeral_system_prompt=agent.system_prompt` to `AIAgent(...)`
  - Pass `enabled_toolsets=agent.enabled_toolsets or None` (empty list → `None` so hermes uses defaults).
  - Stash `self._workspace_dir = agent.workspace_dir`.

`run_turn()`:
- Wrap the `_blocking_chat` call: before calling, snapshot `os.environ.get("TERMINAL_CWD")`, then `os.environ["TERMINAL_CWD"] = self._workspace_dir`. In a `try/finally`, restore the snapshot (delete if it was unset).
- Ensure `self._workspace_dir` exists (`Path(...).mkdir(parents=True, exist_ok=True)`) on first turn.

`make_real_runner(settings, session_id, agent_id)`:
- If `agent_id is not None`, fetch via `AgentStore(settings).get(agent_id)`. If not found → still construct without persona (and log a warning) so the connection isn't fatal.
- Wire to `_HermesAgentAdapter`.

`__main__.py` updates the bind line:
```python
ws_chat.bind_runner_factory(
    lambda sid, aid: make_real_runner(settings, sid, aid)
)
```

### 4.6 Backend tests (new files under `backend/tests/`)

- `test_agent_store.py` — CRUD round-trip, atomic write (simulate crash mid-write), seed idempotence, workspace dir creation, rotate.
- `test_agents_api.py` — happy path + 404 + 422 (validation) + bearer auth.
- `test_toolsets_api.py` — mock `TOOLSETS` import, verify projection.
- `test_hermes_factory.py` — fake `AIAgent` class capturing kwargs; assert `ephemeral_system_prompt`, `model`, `enabled_toolsets` are passed when agent supplied; assert `TERMINAL_CWD` set during turn and restored after.

## 5. Frontend

### 5.1 New `lib/agents-rest.ts`

Reuses `call<T>` from `hermes-rest.ts`:

```ts
export interface Agent { /* matches §3.1 */ }
export interface AgentCreate { /* §3.1 minus id/current_session_id/created_at; workspace_dir optional */ }
export interface Toolset { name: string; description: string; tools: string[] }

export function listAgents(token: string): Promise<Agent[]>;
export function createAgent(payload: AgentCreate, token: string): Promise<Agent>;
export function deleteAgent(id: string, token: string): Promise<void>;
export function rotateAgentSession(id: string, token: string): Promise<{ session_id: string }>;
export function listToolsets(token: string): Promise<Toolset[]>;
```

### 5.2 `ClawProvider` extensions (`lib/store.tsx`)

New state:
- `agents: Agent[]`
- `activeAgentId: string | null` — persisted at `localStorage["clawx.active_agent"]`
- `toolsets: Toolset[]`
- `loadingHistory: boolean`

New actions:
- `refreshAgents()`, `refreshToolsets()` — called once on token-bootstrap.
- `selectAgent(id)`
- `createAgent(payload)` — POST → push → auto-select.
- `deleteAgent(id)` — DELETE → splice; if `id === activeAgentId`, fall back to `agents[0]?.id ?? null`.
- `newConversation()` — `rotateAgentSession(activeAgentId)` → patch the agent in state → clear `chat` → effect reconnects WS with new session_id.

WS connect effect change:
- Dependencies: `[token, wsUrl, activeAgentId, activeAgent?.current_session_id]`
- URL: `${wsUrl}?session_id=<sid>&agent_id=<aid>`
- On `activeAgentId` change: also load history via `getSession(current_session_id)` → `chat.replaceMessages(...)`.
- 404 from `getSession` → treat as empty history (no error toast).

### 5.3 `ChatStore.replaceMessages(msgs: SessionMessage[])`

Maps server messages to `ChatMessage` (role, content, generated id, ts=now). Replaces `messages` wholesale, clears `typing` and `lastError`, emits.

### 5.4 Replace mock `AgentSidebar`

- Source: `claw.agents`.
- Active row: `claw.activeAgentId`.
- Click row → `claw.selectAgent(id)`.
- Hover row → trash icon appears in top-right; click → `window.confirm("删除 Agent 「{name}」？此操作不可撤销。")` → `claw.deleteAgent(id)`.
- "+" → opens `<CreateAgentModal />`.
- Status text per row:
  - Row is active **and** `claw.chat.typing` → `"Running"` (green dot).
  - Otherwise → `"Idle"` (gray dot).
- Icon component: lookup table mapping the `icon` string to the lucide React component. Whitelist of 12 icons (see §5.5).

### 5.5 New `components/CreateAgentModal.tsx`

Built on the existing `Dialog` component. Form fields:

1. **Name** — text, required.
2. **Description** — text, optional.
3. **Color + icon** — two grids of swatches.
   - 12 colors: `#5749F4 #3B82F6 #EC4899 #F59E0B #22C55E #EF4444 #14B8A6 #8B5CF6 #F97316 #06B6D4 #84CC16 #6366F1`
   - 12 icons: `Code2 Search PenTool BarChart3 Bot MessageSquare FileText Lightbulb Sparkles Database Globe Wrench`
4. **System Prompt** — textarea, required, min 8 rows.
5. **Model** — single-select pill row (radio behavior, exactly one selected): `[ 跟随全局 ]` (default) `[ 自定义 ]`. When `自定义` is selected, a select appears below with options `Sonnet 4.6, Opus 4.6, Haiku 4.5, GLM-4.5-Air, GLM-4.5-Plus, GPT-4o, DeepSeek-V3, 自定义...`. Choosing `自定义...` reveals a free-text input. The submitted payload's `model` is `null` when "跟随全局" is active, otherwise the chosen string.
6. **工具集** (collapsed by default under a "高级" disclosure) — checkbox grid driven by `claw.toolsets`. Default: all checked. Buttons: `全选` / `全不选`.

Submit:
- Inline validation on `name` and `system_prompt`.
- Build `AgentCreate` payload (omit `model` field if "跟随全局" — frontend sends `null`).
- Call `claw.createAgent(payload)`. On success: close modal. On failure: top-of-modal error banner with backend `message`.

### 5.6 `ChatPage` adjustments

- Reads `claw.activeAgent` (derived: `agents.find(a => a.id === activeAgentId)`).
- If no `activeAgent` (e.g. agents list empty) → shows "请在左侧选择一个 Agent".
- Top header row layout: `[ 对话 / 产物 tabs ]   ··· flex-spacer ···   [ 新对话 IconButton (RotateCcw) ]`.
- "新对话" button → `window.confirm` → `claw.newConversation()`.
- Welcome screen: when `messages.length === 0`, render the agent's name/description/icon/color (existing `ChatWelcome` extended to accept props with these fields; falls back to MaxClaw defaults when no agent).

### 5.7 Frontend tests (new + extensions)

- `lib/__tests__/agents-rest.test.ts` — fetch URL/method/body/auth.
- `lib/__tests__/store.test.tsx` — extend with: agents bootstrap, selectAgent loads history + reconnects WS, createAgent appends + selects, deleteAgent fall-through, newConversation rotates session_id and reconnects.
- `components/__tests__/CreateAgentModal.test.tsx` — required-field validation, 工具集 default-all-checked, submit calls store.createAgent with the right payload.
- `components/__tests__/AgentSidebar.test.tsx` — renders `claw.agents`, click selects, "+" opens modal, hover→delete confirms.
- `pages/__tests__/ChatPage.test.tsx` — update existing test to seed an active agent in store; assert "新对话" button visible.

## 6. End-to-end Flow Examples

### 6.1 First app launch (no agents.json)

1. Frontend mounts → reads token from localStorage → `refreshInfo()` returns `{enabled: true}`.
2. Frontend calls `listAgents()`. Backend `AgentStore.list()` finds no file, runs `_seed_if_missing()`, returns 4 seeds.
3. Frontend calls `listToolsets()`. Backend returns ~24 toolsets from hermes registry.
4. `activeAgentId` not in localStorage → store picks `agents[0]` (`code`).
5. WS connects with `?session_id=<seeded_sid>&agent_id=code`.
6. Backend factory builds `AIAgent(model=<global>, ephemeral_system_prompt=<编程助手 prompt>, enabled_toolsets=["terminal","file","skills","debugging","code_execution"])`.
7. User sends "hello" → assistant replies with persona-shaped output.

### 6.2 Switch agent

1. User clicks "研究助手" in sidebar.
2. Store sets `activeAgentId="research"`. WS effect tears down old socket, opens new one with new `agent_id` and that agent's `current_session_id`.
3. Store calls `getSession(current_session_id)`. SQLite has previous turns (or 404 → empty). `chat.replaceMessages(...)` populates UI.
4. Subsequent sends go through the new agent.

### 6.3 New conversation

1. User clicks "新对话" → confirm.
2. Store calls `rotateAgentSession(activeAgentId)`. Backend writes new `current_session_id` to JSON, returns it.
3. Store patches the agent in local state → WS effect reconnects on the new session_id (deps changed).
4. UI clears messages; old session row stays in SQLite.

### 6.4 Create agent

1. User clicks "+" → fills form → submits.
2. Frontend POSTs `/api/agents`. Backend assigns `id`, generates `current_session_id`, creates `~/.hermes/workspaces/<id>/`.
3. Returned agent appended to `claw.agents`; auto-selected → flow §6.2.

### 6.5 Delete agent

1. User hovers a row → clicks trash → confirms.
2. Frontend DELETEs. Backend removes the entry; workspace dir untouched.
3. Frontend splices; if it was active, fall back to `agents[0]`.

## 7. Out-of-scope risks accepted for v1

- **Concurrent turns across agents**: `TERMINAL_CWD` is process-global, so two simultaneous turns with different workspaces would corrupt each other. Mitigation: single-tab single-active-agent assumption. If the user opens two tabs, last-wins on cwd. Future fix: subprocess-per-agent or thread-local cwd in hermes-agent (upstream patch).
- **Workspace dir orphans**: deleting agents leaves directories. Not critical for a local dev tool; user can `rm -rf` if desired.
- **Stale active agent in localStorage**: if user deletes the active agent in another window, on next mount we fall back to first available.
- **No optimistic UI** on create/delete: form waits for backend response. Acceptable for local-host latency.

## 8. Implementation order (preview for the plan phase)

1. Backend: `agent_store.py` + tests.
2. Backend: `api/agents.py` + `api/toolsets.py` + tests.
3. Backend: WS + factory persona injection + tests.
4. Frontend: `agents-rest.ts` + store extensions + ChatStore.replaceMessages.
5. Frontend: real `AgentSidebar`.
6. Frontend: `CreateAgentModal`.
7. Frontend: `ChatPage` "新对话" + agent-aware welcome.
8. End-to-end smoke in browser; restart-and-resume verification.

The actual implementation plan (with verification gates between steps) will be produced by `writing-plans`.
