# Hermes Agent Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the vendored picoclaw Go backend with a Python backend built on top of [NousResearch/hermes-agent](https://github.com/nousresearch/hermes-agent), while preserving the existing React frontend's interaction logic (chat store, message-id merge, reconnect, Pico wire protocol shapes).

**Architecture:** `apps/clawx-gui/` (React/Vite/TS) stays. `backend/` is rewritten as a Python FastAPI adapter (`hermes_bridge`) that (1) imports hermes-agent as a library dependency, (2) exposes a REST + WebSocket surface on `127.0.0.1:18800` in the same shape the current frontend already speaks (`/api/*`, WS with `message.send`/`message.create`/`message.update`/`typing.*`/`error`/`pong`), and (3) maps hermes's agent turn loop, skills, toolsets, and session memory onto that surface. Paths are renamed `/api/pico/*` → `/api/hermes/*` and `/pico/ws` → `/hermes/ws`; frontend symbols `Pico*` → `Hermes*`. Wire message shapes are unchanged.

**Tech Stack:**
- Backend: Python ≥ 3.11, FastAPI, uvicorn, `uv` package manager, hermes-agent as git dependency, Pydantic v2, pytest + pytest-asyncio + httpx for tests.
- Frontend: unchanged (React 19 + Vite 6 + TS 5 + vitest 4).
- Dev orchestration: `concurrently` via root `pnpm dev` — swaps `go run` for `uv run`.

---

## Critical Context for the Engineer

Before starting Task 1, read these files in this order to understand the existing contract you must preserve:

1. `docs/arch/architecture.md` — current v5.0 picoclaw architecture (will be replaced in Task 0.1).
2. `docs/arch/api-design.md` — current Pico wire protocol (will be rewritten in Task 0.2).
3. `apps/clawx-gui/src/lib/pico-types.ts` — **the contract your new backend must match at the wire level** (you keep the message shapes; only path prefix changes).
4. `apps/clawx-gui/src/lib/pico-rest.ts` — REST client → endpoints your backend must serve.
5. `apps/clawx-gui/src/lib/pico-socket.ts` — WS client; note `Sec-WebSocket-Protocol: token.<…>` subprotocol auth. Your Python server must accept this.
6. `apps/clawx-gui/src/lib/chat-store.ts` — how the UI merges `message.create`/`message.update` by `message_id`. Your hermes bridge must emit `message_id`-stable frames.
7. `apps/clawx-gui/src/lib/store.tsx` — session lifecycle and token flow.

### Concept mapping (picoclaw → hermes-agent)

| Picoclaw concept | Hermes-agent equivalent | Notes |
|---|---|---|
| `dashboardToken` (stdout on launcher startup) | Env `HERMES_LAUNCHER_TOKEN` or generated & printed | We reproduce the same UX |
| `/api/pico/info` | `/api/hermes/info` | Same shape `{configured, enabled, ws_url}` |
| WS `/pico/ws` | WS `/hermes/ws` | Same frame format |
| `channels.pico` enabled flag | Always true once `HERMES_CONFIG_DIR` has a configured model | Hermes has no single "pico channel" toggle; we compute `enabled` from hermes config |
| `/api/sessions` | Hermes session/FTS5 store | See `hermes/gateway/session.py` + `agent/memory_manager.py` |
| `/api/skills` | `~/.hermes/skills/` + hermes `skill_commands.py` | `agent/skill_commands.py` in hermes |
| `/api/tools` | `hermes/toolsets.py` + `hermes/tools/*` | Hermes has an "enabled toolset" concept |
| picoclaw LLM providers | Hermes model adapters in `agent/*_adapter.py` | Configured via `hermes model <provider:model>` or config file |
| MCP client | Hermes MCP via `mcp_serve.py` / tools layer | Continues to work; exposed through /api/tools |

### Useful grep commands

Before writing wrapper code, run these against a cloned hermes-agent repo to locate concrete APIs. The plan references these as `{HERMES}/` — clone `https://github.com/NousResearch/hermes-agent` to a local path and export `HERMES=/path/to/hermes-agent` in your shell.

```bash
# Find the agent entry point (single-turn chat call):
grep -rn "def run" $HERMES/agent/*.py $HERMES/run_agent.py | head
# Find session persistence:
grep -rn "session" $HERMES/gateway/session.py $HERMES/agent/memory_manager.py | head
# Find skill listing:
grep -rn "def.*skill" $HERMES/agent/skill_commands.py $HERMES/agent/skill_utils.py | head
# Find toolset listing/toggle:
grep -rn "def " $HERMES/toolsets.py | head
```

If an API the plan calls for is genuinely absent from hermes (e.g. "list installed skills with descriptions"), implement it inside `hermes_bridge/bridge/*_service.py` by reading the filesystem directly (skills are markdown files in `~/.hermes/skills/`) rather than shelling out to the `hermes` CLI — shelling out couples tests to binary installation.

---

## File Structure

### New `backend/` layout (after migration)

```
backend/
├── pyproject.toml              # uv-managed project; hermes-agent + FastAPI deps
├── uv.lock                     # generated; committed
├── README.md                   # new — dev/run/test commands for Python backend
├── .python-version             # "3.11"
├── hermes_bridge/              # the adapter package
│   ├── __init__.py
│   ├── __main__.py             # `python -m hermes_bridge` → serves uvicorn on :18800
│   ├── app.py                  # FastAPI factory (register routers + middleware)
│   ├── config.py               # Settings (port, token, log level, hermes config dir)
│   ├── auth.py                 # HTTP bearer dep + WS subprotocol auth helper
│   ├── logging_setup.py
│   ├── api/
│   │   ├── __init__.py
│   │   ├── info.py             # GET /api/hermes/info
│   │   ├── sessions.py         # GET/DELETE /api/sessions[/:id]
│   │   ├── skills.py           # GET /api/skills, POST /api/skills/install, DELETE /api/skills/:name
│   │   └── tools.py            # GET /api/tools, PUT /api/tools/:name/state
│   ├── ws/
│   │   ├── __init__.py
│   │   ├── protocol.py         # Pydantic models mirroring pico-types.ts
│   │   └── chat.py             # WS /hermes/ws endpoint
│   └── bridge/
│       ├── __init__.py
│       ├── hermes_runner.py    # one HermesRunner per WS session; wraps agent turn loop
│       ├── session_store.py    # reads/writes hermes session store
│       ├── skill_service.py    # filesystem-backed skills listing
│       └── tool_service.py     # wraps hermes toolsets for list/toggle
├── scripts/
│   └── init_config.py          # replaces Go scripts/init-config; bootstraps ~/.hermes/
└── tests/
    ├── conftest.py             # fastapi TestClient + async fixtures
    ├── test_auth.py
    ├── test_info.py
    ├── test_sessions.py
    ├── test_skills.py
    ├── test_tools.py
    ├── test_ws_protocol.py
    └── test_ws_chat.py
```

**Responsibility split rationale:**
- `api/` is thin HTTP routing + Pydantic schema; all logic lives in `bridge/*_service.py` so it can be unit-tested without a TestClient.
- `ws/chat.py` handles framing + auth + reconnect; actual turn execution is in `bridge/hermes_runner.py` so WS tests can mock the runner.
- `bridge/` never imports from `api/` or `ws/` — one-way dependency.
- `hermes_runner.py` is the only file that imports hermes-agent internals, so if upstream restructures we have one file to patch.

### Frontend rename (Task 7)

| Old | New |
|---|---|
| `apps/clawx-gui/src/lib/pico-types.ts` | `apps/clawx-gui/src/lib/hermes-types.ts` |
| `apps/clawx-gui/src/lib/pico-rest.ts` | `apps/clawx-gui/src/lib/hermes-rest.ts` |
| `apps/clawx-gui/src/lib/pico-socket.ts` | `apps/clawx-gui/src/lib/hermes-socket.ts` |
| `PicoMessage`, `PicoApiError`, `PicoSocket`, `fetchPicoInfo`, `PicoInfo` | `HermesMessage`, `HermesApiError`, `HermesSocket`, `fetchHermesInfo`, `HermesInfo` |
| `localStorage["clawx.dashboard_token"]` | unchanged (same key to preserve logged-in users) |

Message type strings (`message.send`, `message.create`, `typing.start`, etc.) remain verbatim.

### Files to delete (Task 8)

- Entire `backend/` Go tree (replaced in one atomic commit after Python backend is green)
- `Cargo.lock` (relic of the Rust era, no longer referenced)
- `target/` (Rust build artefacts)
- `.agents/`, `rules/rust.md` if present — already-cleaned historical artefacts; audit and drop anything still referencing picoclaw or Rust.

---

## Phase 0 — Architecture Documentation (FIRST, per user request)

No code changes in this phase. The engineer updates docs so Phase 1+ has a blueprint.

---

### Task 0.1: Rewrite `docs/arch/architecture.md` to v6.0

**Files:**
- Modify: `docs/arch/architecture.md` (full rewrite)

- [ ] **Step 1: Read the current v5.0 doc to know what you are replacing**

Run: `cat docs/arch/architecture.md`

Expected: v5.0 describing vendored picoclaw Go backend on :18800.

- [ ] **Step 2: Replace the file with v6.0 content**

Write the file with exactly this content:

````markdown
# ClawX Architecture v6.0

**日期:** 2026-04-21

> ClawX v6.0 将后端从 vendored [sipeed/picoclaw](https://github.com/sipeed/picoclaw)（Go）切换为 [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent)（Python），并通过一个薄适配层 `hermes_bridge` 暴露前端已依赖的 REST + WebSocket 契约。前端交互逻辑（会话状态机、`message_id` 合并渲染、子协议 token 鉴权、断线重连）保留不变，仅完成 `pico-*` → `hermes-*` 的重命名。

---

## 1. 架构总览

```
┌──────────────────────────────────────────────────────────────────────┐
│ Browser (any modern browser)                                         │
│   ClawX Web UI (React + Vite + TypeScript) — UNCHANGED logic         │
│     ├── ChatPage          (hermes-socket.ts → WS /hermes/ws)          │
│     ├── ConnectorsPage    (hermes-rest.ts  → /api/skills /api/tools) │
│     └── SettingsPage      (dashboard token paste + /api/hermes/info) │
└─────────┬────────────────────────────────────┬───────────────────────┘
          │ Vite dev (1420) proxies            │ Vite proxies WS
          │ /api/* → 18800                     │ /hermes/ws → 18800
          ▼                                    ▼
┌──────────────────────────────────────────────────────────────────────┐
│ hermes_bridge  (Python FastAPI; uvicorn on 127.0.0.1:18800)          │
│                                                                      │
│   HTTP :18800 — 唯一入口                                             │
│   • Auth     : dashboard token (Authorization: Bearer / Sec-WebSocket│
│                -Protocol: token.<…>)                                  │
│   • REST     : /api/hermes/info, /api/sessions, /api/skills,         │
│                /api/tools                                             │
│   • WS       : /hermes/ws?session_id=<uuid>                           │
│                (message.send / message.create / message.update /      │
│                 typing.start|stop / error / pong — 和旧 Pico 协议    │
│                 帧级兼容)                                             │
│                                                                      │
│   进程内嵌  hermes-agent (Python library)：                           │
│   • Agent turn loop                 → agent/ + run_agent.py           │
│   • Skills (markdown, ~/.hermes/skills/openclaw-imports/)             │
│   • Toolsets (40+ built-in, toggle via toolsets.py)                   │
│   • MCP clients (mcp_serve.py)                                        │
│   • Session memory + FTS5 search    → agent/memory_manager.py         │
│   • 30+ LLM 适配器 (Anthropic/OpenAI/Nous/OpenRouter/Ollama/…)        │
└──────────────────────────────────────────────────────────────────────┘
```

**单一仓库，两个子项目:** `apps/clawx-gui/` (前端) + `backend/` (Python hermes_bridge)。

---

## 2. 仓库布局 (v6.0)

```
frank_claw/
├── apps/
│   └── clawx-gui/                  # 前端（不变的 logic，仅改名）
│       ├── src/
│       │   ├── pages/              # Chat/Connectors/Settings (unchanged)
│       │   ├── components/
│       │   ├── lib/
│       │   │   ├── hermes-rest.ts  # (was pico-rest.ts)
│       │   │   ├── hermes-socket.ts# (was pico-socket.ts)
│       │   │   ├── hermes-types.ts # (was pico-types.ts)
│       │   │   ├── chat-store.ts   # unchanged
│       │   │   └── store.tsx       # unchanged logic, only imports renamed
│       ├── vite.config.ts          # proxy: /api/* + /hermes/ws → :18800
│       └── package.json
├── backend/                        # Python — REWRITTEN for v6.0
│   ├── pyproject.toml              # uv-managed project
│   ├── uv.lock
│   ├── hermes_bridge/              # FastAPI adapter
│   │   ├── app.py
│   │   ├── config.py
│   │   ├── auth.py
│   │   ├── api/                    # info, sessions, skills, tools
│   │   ├── ws/                     # protocol, chat
│   │   └── bridge/                 # hermes_runner, session_store, skill/tool services
│   ├── scripts/init_config.py
│   └── tests/                      # pytest suite
├── docs/
│   ├── arch/                       # v6.0 docs (this file, api-design, decisions)
│   └── superpowers/plans/          # 2026-04-21-hermes-agent-migration.md
├── package.json                    # pnpm + concurrently (dev: uvicorn + vite)
├── README.md
└── AGENTS.md
```

---

## 3. 协议层

详见 [api-design.md](./api-design.md)。摘要：

### 3.1 WebSocket (聊天主通道)

| 项 | 值 |
|---|---|
| URL (dev) | `ws://localhost:1420/hermes/ws?session_id=<uuid>` (Vite proxy) |
| URL (直连) | `ws://127.0.0.1:18800/hermes/ws?session_id=<uuid>` |
| 鉴权 | `Sec-WebSocket-Protocol: token.<dashboardToken>` |
| Token 来源 | `HERMES_LAUNCHER_TOKEN` 环境变量，或首次启动时 `hermes_bridge` 自动生成并写入 `~/.hermes/launcher-token` 同时 stdout 打印 `dashboardToken: <…>` |
| 消息封套 | `{ type, id?, session_id?, timestamp?, payload? }` — 与 v5.0 完全一致 |
| 类型矩阵 | 客户端→服务端 `message.send` / `media.send` / `ping`；服务端→客户端 `message.create` / `message.update` / `media.create` / `typing.start` / `typing.stop` / `error` / `pong` |
| 流式策略 | 无 token 级 streaming；服务端每条消息完成后下发 `message.create`；思考过程用 `payload.thought: true` |
| 消息合并 | 同 `payload.message_id` 的 `message.create` + `message.update` 在前端按 id 合并 |

### 3.2 REST (管理面)

所有 `/api/*` 请求带 `Authorization: Bearer <dashboardToken>`。

| 用途 | 端点 |
|---|---|
| 验证 token + 拿 ws_url | `GET /api/hermes/info` → `{configured, enabled, ws_url}` |
| 会话 列表 / 详情 / 删除 | `GET /api/sessions[?offset&limit]` / `GET /api/sessions/:id` / `DELETE /api/sessions/:id` |
| Skills 浏览 / 安装 / 卸载 | `GET /api/skills` → `{skills: [...]}` / `POST /api/skills/install` / `DELETE /api/skills/:name` |
| Tools 列出 / 启停 | `GET /api/tools` → `{tools: [...]}`，每项 `status: "enabled"\|"disabled"\|"blocked"` / `PUT /api/tools/:name/state` body `{enabled: bool}` |

---

## 4. 前端职责边界 (不变)

| 区域 | 能力 |
|---|---|
| ChatPage | 基于 WS 建立会话、发送消息、渲染 `message.create`/`message.update`、显示 typing / thought |
| ConnectorsPage | 列出 skills + tools，切换 tool 启停 |
| SettingsPage | 粘贴 / 清除 dashboard token、刷新连接信息 |
| 本地配置 | LLM provider、MCP、toolset 在 `~/.hermes/` 由 hermes-agent 负责 |

---

## 5. 部署形态

### 5.1 开发模式

需要：Python ≥ 3.11 + `uv`；Node ≥ 22 + pnpm ≥ 10；至少一个 hermes 支持的 LLM provider 凭证（Anthropic/OpenAI/OpenRouter/Nous/Ollama…）。

```bash
# 一次性：在 backend/ 安装 Python deps
cd backend && uv sync && cd ..

# 一次性：bootstrap hermes 配置
uv run --project backend python backend/scripts/init_config.py

# 日常：根目录一条命令并行起前后端
pnpm dev    # concurrently 跑：
            #   1) uv run --project backend python -m hermes_bridge  → :18800
            #   2) pnpm --filter clawx-gui dev                       → :1420
```

打开 `http://localhost:1420`，从 `hermes_bridge` 终端复制 `dashboardToken: …` 粘到 SettingsPage。

### 5.2 生产 / 单机模式

```bash
pnpm build                   # 前端产出 apps/clawx-gui/dist/
uv run --project backend python -m hermes_bridge \
    --webroot ./apps/clawx-gui/dist \
    --no-browser
```

`hermes_bridge` 以 `StaticFiles` 挂载 `--webroot`，同进程托管前端静态资产 + API。无需 docker / nginx。

---

## 6. 技术栈

| 层 | 技术 | 备注 |
|---|---|---|
| 前端 UI | React 19 + TypeScript 5 | `apps/clawx-gui/` |
| 前端构建 | Vite 6 | dev server 1420，proxy `/api` + `/hermes/ws` |
| 路由 | react-router-dom 7 | ChatPage / ConnectorsPage / SettingsPage |
| Markdown | react-markdown + remark-gfm + highlight.js | |
| 前端测试 | vitest 4 + @testing-library/react + jsdom | |
| 后端语言 | Python ≥ 3.11 | `backend/hermes_bridge/` |
| 后端框架 | FastAPI + uvicorn | WebSocket + HTTP |
| 后端包管理 | `uv` | 与 hermes-agent 上游一致 |
| 后端测试 | pytest + pytest-asyncio + httpx | |
| 底层 agent | hermes-agent (pinned git ref) | 作为 Python 依赖引入 |
| 进程管理 | concurrently (dev)；单 uvicorn 进程 (prod) | |

---

## 7. 安全模型

1. 所有流量走 loopback (`127.0.0.1:18800`)
2. Dashboard token 只在 `localStorage["clawx.dashboard_token"]` 暂存；REST 用 Bearer、WS 用 Sec-WebSocket-Protocol 子协议；不进 cookie，不回传外部域；UI 掩码展示
3. 前端不直接持有 LLM API Key —— 全部托管在 `~/.hermes/` (hermes-agent 管辖)
4. 前端不实现沙箱 / 命令执行；所有 tool 调用在 hermes-agent 内完成

---

## 8. 上游同步策略

hermes-agent 以 **固定 git ref** 的方式在 `backend/pyproject.toml` 中引入（`hermes-agent @ git+https://github.com/NousResearch/hermes-agent@<sha>`）。升级流程：

1. 改 `pyproject.toml` 的 SHA，`uv lock` 重新锁定
2. 跑 `uv run pytest` 全套通过
3. 跑前端 `pnpm test` 通过
4. 人工烟测一次端到端聊天

如果某次上游升级破坏了 `hermes_bridge/bridge/hermes_runner.py` 内对 hermes 内部 API 的调用，修 `hermes_runner.py`，其它文件不动 —— 这是把所有"上游脆弱点"集中在一处的刻意设计。

---

## 9. 历史文档

- v5.0 `architecture.md`（本次被覆盖）— 见 git history
- v4.2 Rust-era 文档（均为 DEPRECATED）：
  - [autonomy-architecture.md](./autonomy-architecture.md)
  - [memory-architecture.md](./memory-architecture.md)
  - [security-architecture.md](./security-architecture.md)
  - [data-model.md](./data-model.md)
  - [crate-dependency-graph.md](./crate-dependency-graph.md)
````

- [ ] **Step 3: Verify the replacement**

Run: `head -5 docs/arch/architecture.md`
Expected output starts with `# ClawX Architecture v6.0`.

- [ ] **Step 4: Commit**

```bash
git add docs/arch/architecture.md
git commit -m "docs(arch): v6.0 — hermes-agent backend replaces picoclaw"
```

---

### Task 0.2: Rewrite `docs/arch/api-design.md` to v6.0

**Files:**
- Modify: `docs/arch/api-design.md` (full rewrite)

- [ ] **Step 1: Replace the file**

Write the file with exactly this content:

````markdown
# ClawX API 设计 v6.0 (hermes_bridge, owned contract)

**日期:** 2026-04-21 | **对应架构:** v6.0 | **取代:** v5.0

> 自 v6.0 起，本仓库以 `backend/hermes_bridge/` 为 API 权威。hermes-agent 作为 Python library 在同进程内被适配层调用；契约仍然是我们自己拥有并维护的。

---

## 1. 总览

| 项 | 值 |
|---|---|
| 后端进程 | `hermes_bridge` (FastAPI + uvicorn)，入口 `python -m hermes_bridge` |
| 监听 | `127.0.0.1:18800` (`--port`, default `18800`) — 唯一 HTTP 入口 |
| 鉴权根 | Dashboard Token，env `HERMES_LAUNCHER_TOKEN` 优先；未设置则启动时生成并写 `~/.hermes/launcher-token`，同时 stdout 打印 `dashboardToken: <…>` |
| 鉴权头 | `Authorization: Bearer <token>` (REST) |
| WS 鉴权 | `Sec-WebSocket-Protocol: token.<token>` (子协议) |
| Token 发现 | `GET /api/hermes/info` → `{configured, enabled, ws_url}` |
| 传输 | HTTP/JSON + WebSocket |

---

## 2. 鉴权

**Bootstrap（首次访问）**：

1. 启动后端：`uv run --project backend python -m hermes_bridge`，从 stdout 复制 `dashboardToken: <…>`（或 env `HERMES_LAUNCHER_TOKEN` 固定）。
2. 前端 SettingsPage 粘贴 token，存 `localStorage["clawx.dashboard_token"]`。
3. 之后所有 `/api/*` 请求带 `Authorization: Bearer <token>`。
4. 前端 `GET /api/hermes/info` 验证 token 有效并拿到 `ws_url`；若 `enabled === false`，提示用户配置 LLM provider 并重启 `hermes_bridge`。

**WebSocket**: `new WebSocket(ws_url + "?session_id=<uuid>", ["token." + token])`。

> 浏览器 WS API 无法自定义 header，因此 `hermes_bridge` **原生支持** `Sec-WebSocket-Protocol: token.<…>` 子协议；不像 picoclaw 那样需要 patch 上游。

REST 不要用 query 传 token（易泄漏到日志）。

---

## 3. WebSocket 协议

### 3.1 端点

```
ws[s]://127.0.0.1:18800/hermes/ws?session_id=<uuid>
Sec-WebSocket-Protocol: token.<token>
```

`session_id` 由前端 `crypto.randomUUID()` 生成。`hermes_bridge` 为每个 `session_id` 维护一个 `HermesRunner`；同一 `session_id` 并发连接会共享 runner（后连接的 socket 加入广播目标）。

### 3.2 消息封套

```ts
interface HermesMessage {
  type: HermesMessageType;
  id?: string;           // client-generated request id; 服务端在 error.payload.request_id 回显
  session_id?: string;
  timestamp?: number;    // unix millis
  payload?: Record<string, unknown>;
}
```

### 3.3 类型矩阵

| 方向 | type | payload |
|---|---|---|
| C→S | `message.send` | `{ content: string, media?: string \| object \| Array<dataURL> }` |
| C→S | `media.send` | （仅文件场景，本期不实现） |
| C→S | `ping` | `{}` |
| S→C | `message.create` | `{ message_id: string, content: string, thought?: boolean }` |
| S→C | `message.update` | `{ message_id: string, content: string, thought?: boolean }` |
| S→C | `media.create` | （本期忽略） |
| S→C | `typing.start` | `{}` |
| S→C | `typing.stop` | `{}` |
| S→C | `error` | `{ code: string, message: string, request_id?: string }` |
| S→C | `pong` | `{}` (echo client `id`) |

### 3.4 渲染规则

- 无 token 级 streaming；每条消息完成后一次 `message.create`。
- 同 `message_id` 后续可有零或多个 `message.update`；前端按 id 合并替换 `content`。
- `payload.thought === true` → "思考过程" 次级气泡；否则视为最终回复。
- `typing.stop` 是 turn 结束辅助信号；没有独立的 `done`。
- `media.create` 本期不实现。

### 3.5 错误处理

- 收到 `error` 时，若 `payload.request_id` 匹配某条乐观渲染的 `message.send.id`，前端回滚该用户消息。
- 关闭码 `1000` = 正常；其他触发指数退避重连（500ms → 30s 上限），复用同 `session_id` 继续历史。

### 3.6 心跳

客户端每 25s 发 `{type:"ping", id:<nonce>}`，60s 内无 `pong` 则断开重连。服务端 `pong` 必须回显客户端 `id`。

### 3.7 hermes → wire frame 映射

`HermesRunner` 在每个 agent turn 中会依次发出：

1. 用户消息进入 → `typing.start`
2. hermes 的每个思考步骤（tool call、中间推理）→ 一条 `message.create`，`payload.thought=true`，`payload.message_id` 来自 hermes turn 内部 id
3. hermes 的最终 assistant 消息 → 一条 `message.create`，`thought=false`
4. turn 结束 → `typing.stop`

若 hermes 事件流不含显式 `turn_end`，以"最终 assistant 消息已发出且下一个 `message.send` 尚未到达"作为边界，由 `HermesRunner` 合成 `typing.stop`。

---

## 4. REST 端点

### 4.1 Info

| Method | Path | Resp |
|---|---|---|
| GET | `/api/hermes/info` | `{ configured: boolean, enabled: boolean, ws_url: string }` |

`configured` = hermes 至少有一个可用 LLM 适配器；`enabled` = `hermes_bridge` 当前能接受聊天（等价于 `configured` 且 `HermesRunner` 工厂初始化成功）。

### 4.2 Sessions

| Method | Path | Resp |
|---|---|---|
| GET | `/api/sessions?offset=0&limit=50` | `SessionSummary[]` |
| GET | `/api/sessions/:id` | `SessionDetail` |
| DELETE | `/api/sessions/:id` | 204 |

```ts
interface SessionSummary {
  id: string;
  title: string;
  preview: string;
  message_count: number;
  created: number;    // unix millis
  updated: number;    // unix millis
}

interface SessionDetail extends SessionSummary {
  messages: Array<{
    role: "user" | "assistant" | "system";
    content: string;
    media?: unknown;
  }>;
  summary: string;
}
```

数据来源：hermes 的 session 存储（FTS5 SQLite，位于 `~/.hermes/`，由 hermes-agent 自管）。`hermes_bridge.bridge.session_store.SessionStore` 是唯一读/删封装。

### 4.3 Skills

| Method | Path | 用途 |
|---|---|---|
| GET | `/api/skills` | 列出已安装 (body `{skills: SkillInfo[]}`) |
| POST | `/api/skills/install` | body `{name}` — 从 ClawHub / agentskills.io 安装 |
| DELETE | `/api/skills/:name` | 卸载（仅限用户 skills 目录内） |

```ts
interface SkillInfo {
  name: string;
  description?: string;
  installed?: boolean;
}
```

skills 实际目录：`~/.hermes/skills/` (含 `openclaw-imports/` 子目录的用户迁入包)。SKILL.md 首部 frontmatter 的 `description` 字段作为列表展示。

### 4.4 Tools

| Method | Path | 用途 |
|---|---|---|
| GET | `/api/tools` | 列出 + 启用状态 (body `{tools: ToolInfo[]}`) |
| PUT | `/api/tools/:name/state` | body `{enabled: boolean}` |

```ts
type ToolStatus = "enabled" | "disabled" | "blocked";

interface ToolInfo {
  name: string;
  status: ToolStatus;         // server-authoritative
  description?: string;
  category?: string;
  config_key?: string;
  reason_code?: string;       // 只在 blocked 时出现（如缺 API key）
}
```

`enabled` 布尔字段由前端派生 (`status === "enabled"`)，保持与 v5 前端一致，**不在 wire format 里传**。

数据来源：hermes `toolsets.py` 的启用列表 + `tools/*` 的描述元数据。`blocked` 状态 = 工具存在但运行时依赖缺失（如 MCP server 未配置、API key 缺）。

---

## 5. 错误响应

```ts
interface ErrorBody {
  message: string;
  code?: string;     // 机器可读类别
  details?: Record<string, unknown>;
}
```

HTTP：401 (bad token) / 404 (unknown session|skill|tool) / 409 (tool toggle conflicts) / 500。

WS：上表 `error` 帧；同时可能以 close code 非-1000 断线（客户端应重连）。

---

## 6. 实现引用

| 文件 | 内容 |
|---|---|
| `backend/hermes_bridge/ws/protocol.py` | Pydantic v2 models 对齐 `apps/clawx-gui/src/lib/hermes-types.ts` |
| `backend/hermes_bridge/ws/chat.py` | `/hermes/ws` 端点（子协议鉴权 + 心跳 + 广播） |
| `backend/hermes_bridge/auth.py` | Bearer dep + WS 子协议校验 |
| `backend/hermes_bridge/api/*.py` | REST handler 薄层（仅 HTTP；逻辑在 bridge/） |
| `backend/hermes_bridge/bridge/hermes_runner.py` | 唯一导入 hermes-agent 内部符号的文件；升级时先改这里 |

---

## 7. 演进策略

- 协议字段任何变化都同步 `apps/clawx-gui/src/lib/hermes-types.ts` + 对应 vitest 协议合约测试 + `backend/tests/test_ws_protocol.py`。
- hermes-agent 升级：改 `pyproject.toml` 的 git SHA，`uv lock`，跑 `uv run pytest` + `pnpm test`。冲突只改 `hermes_runner.py`。
- 协议破坏性变更：本仓库 minor 抬升，同步记入 `docs/arch/decisions.md`。
````

- [ ] **Step 2: Commit**

```bash
git add docs/arch/api-design.md
git commit -m "docs(api): v6.0 — rewrite contract for hermes_bridge"
```

---

### Task 0.3: Add ADR-038 to `docs/arch/decisions.md`

**Files:**
- Modify: `docs/arch/decisions.md` (append new ADR)

- [ ] **Step 1: Locate insertion point**

Run: `grep -n "^## ADR-" docs/arch/decisions.md | tail`
Expected: output lists ADR-037 as the last one.

- [ ] **Step 2: Append ADR-038**

Append exactly this block to the **end** of `docs/arch/decisions.md`:

```markdown

---

## ADR-038: 2026-04-21 — 后端从 vendored picoclaw 切换到 hermes-agent

**决策:** 删除 `backend/` 下 vendored 的 Go picoclaw；引入 [NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent) 作为后端 agent 内核，并通过一个 Python FastAPI 适配层 `hermes_bridge` 对前端继续暴露 v5.0 已定义的 Pico 契约（路径改名 `/api/pico/*` → `/api/hermes/*`，`/pico/ws` → `/hermes/ws`；消息帧格式不变）。

**理由:**
1. hermes-agent 的能力矩阵（skills 自生成、FTS5 session 搜索、40+ 内置工具、MCP、Nous Portal/OpenRouter/OpenAI/Anthropic/Ollama/… 模型无缝切换）已超出 picoclaw 的当前提供面，而且其发展节奏更可预期。
2. picoclaw 的 WS 鉴权需要我们在上游之上持续维护本地 patch（`Sec-WebSocket-Protocol: token.<…>`），而 `hermes_bridge` 作为我们自有进程可直接原生支持，不再需要 patch 管理。
3. 前端 `chat-store.ts` + `PicoSocket` + `Pico*` 类型已经围绕这套 wire 协议稳定下来，保留契约意味着前端只改文件名和 import，不改 runtime 行为。
4. hermes-agent 以 Python 为主，`hermes_bridge` 选 FastAPI 可以同时满足：最小化 Python 依赖、原生 WS 支持、TestClient 可驱动的强可测性、与 hermes-agent 相同的 `uv` 工具链。

**不采取的替代方案:**
- **直接在 hermes-agent fork 内新增 HTTP/WS 服务**: 上游合并路径不明，维护本地 fork 负担大。
- **在浏览器直连 hermes TUI 的 PTY**: 放弃 Pico 协议结构化帧的全部好处，合并/重连/thought 气泡等需要重写。
- **保留 picoclaw 再并跑**: 双后端、双鉴权、双配置，收益小于复杂度。

**影响:**
- 删除整个 `backend/` Go 树 + `Cargo.lock` + `target/`。
- 新增 `backend/` Python 项目（`pyproject.toml` + `hermes_bridge/`）。
- 前端 `pico-*` 文件 & 符号更名为 `hermes-*`；wire 格式与 store 行为不变。
- 根 `package.json` 的 dev 脚本从 `go run` 改为 `uv run python -m hermes_bridge`。
- 先前在 ADR-037 v2 中的"vendor picoclaw 源码"策略随本决策作废。

**迁移计划:** `docs/superpowers/plans/2026-04-21-hermes-agent-migration.md`
```

- [ ] **Step 3: Verify**

Run: `grep -c "^## ADR-038" docs/arch/decisions.md`
Expected: `1`

- [ ] **Step 4: Commit**

```bash
git add docs/arch/decisions.md
git commit -m "docs(adr): ADR-038 — hermes-agent replaces vendored picoclaw"
```

---

### Task 0.4: Update `docs/arch/README.md` index

**Files:**
- Modify: `docs/arch/README.md`

- [ ] **Step 1: Read current content**

Run: `cat docs/arch/README.md`

- [ ] **Step 2: Replace top-of-file version marker**

Replace the first heading/line that mentions `v5.0` with `v6.0`, and add a one-line pointer to ADR-038. If the file lists active docs, ensure `architecture.md` and `api-design.md` are listed and both labelled "v6.0 — current". DEPRECATED v4.2 doc list stays as-is.

Concretely: if the README currently reads:

```markdown
# ClawX Architecture Docs (v5.0 current)
```

change to:

```markdown
# ClawX Architecture Docs (v6.0 current)

Current version: v6.0 — hermes_bridge (Python) replaces vendored picoclaw. See [ADR-038](./decisions.md#adr-038-2026-04-21--后端从-vendored-picoclaw-切换到-hermes-agent).
```

Preserve all other links. If the README doesn't exist in that form, just edit the version marker and add the one-line ADR-038 pointer after it.

- [ ] **Step 3: Commit**

```bash
git add docs/arch/README.md
git commit -m "docs(arch): bump index to v6.0"
```

---

## Phase 1 — Python Backend Scaffold (empty skeleton, tests ready)

No picoclaw code is removed yet; we build the new backend alongside in a new directory name first, then swap in Phase 8. To avoid path collision, develop the Python backend in `backend-py/` during Phases 1–7, and rename `backend-py/` → `backend/` (after deleting the Go tree) in Phase 8.1.

---

### Task 1.1: Create `backend-py/` with `pyproject.toml` and `uv` bootstrap

**Files:**
- Create: `backend-py/pyproject.toml`
- Create: `backend-py/.python-version`
- Create: `backend-py/README.md`

- [ ] **Step 1: Verify you have `uv` installed**

Run: `uv --version`
Expected: prints version (`uv 0.4+`). If not installed: `curl -LsSf https://astral.sh/uv/install.sh | sh`.

- [ ] **Step 2: Create `backend-py/` and its pyproject**

```bash
mkdir -p backend-py
```

Write `backend-py/pyproject.toml`:

```toml
[project]
name = "hermes-bridge"
version = "0.1.0"
description = "FastAPI adapter exposing a Pico-compatible REST+WS contract on top of hermes-agent"
requires-python = ">=3.11"
dependencies = [
    "fastapi>=0.115",
    "uvicorn[standard]>=0.32",
    "pydantic>=2.9",
    "pydantic-settings>=2.6",
    "anyio>=4.6",
    # hermes-agent as a git dependency; pin a SHA once you verify the first
    # working commit in Task 3.1. Use `main` for now during bootstrap.
    "hermes-agent @ git+https://github.com/NousResearch/hermes-agent@main",
]

[project.optional-dependencies]
dev = [
    "pytest>=8.3",
    "pytest-asyncio>=0.24",
    "httpx>=0.28",
    "websockets>=13",
    "ruff>=0.7",
]

[project.scripts]
hermes-bridge = "hermes_bridge.__main__:main"

[build-system]
requires = ["hatchling"]
build-backend = "hatchling.build"

[tool.hatch.build.targets.wheel]
packages = ["hermes_bridge"]

[tool.pytest.ini_options]
asyncio_mode = "auto"
testpaths = ["tests"]
```

Write `backend-py/.python-version`:

```
3.11
```

Write `backend-py/README.md`:

```markdown
# hermes_bridge

Python FastAPI adapter that embeds [hermes-agent](https://github.com/NousResearch/hermes-agent) and exposes a Pico-compatible REST + WebSocket API consumed by `apps/clawx-gui/`.

## Dev

```bash
uv sync
uv run python -m hermes_bridge
```

## Test

```bash
uv run pytest
```

## Entrypoint

`python -m hermes_bridge` → uvicorn on `127.0.0.1:18800`.
```

- [ ] **Step 3: Lock and install**

```bash
cd backend-py && uv sync && cd ..
```

Expected: `uv.lock` appears; no errors. If the hermes-agent git install fails, note the first error and either switch to a specific SHA known to be installable or add the missing system deps (hermes-agent may need `git`, `pkg-config`, etc.) then retry.

- [ ] **Step 4: Commit**

```bash
git add backend-py/pyproject.toml backend-py/.python-version backend-py/README.md backend-py/uv.lock
git commit -m "feat(backend-py): scaffold uv project with hermes-agent dep"
```

---

### Task 1.2: Empty package + FastAPI factory + main entry

**Files:**
- Create: `backend-py/hermes_bridge/__init__.py`
- Create: `backend-py/hermes_bridge/config.py`
- Create: `backend-py/hermes_bridge/logging_setup.py`
- Create: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/hermes_bridge/__main__.py`

- [ ] **Step 1: Write failing test that the factory returns a FastAPI app with a health route**

Create `backend-py/tests/__init__.py` (empty) and `backend-py/tests/test_app.py`:

```python
from fastapi.testclient import TestClient
from hermes_bridge.app import create_app


def test_create_app_returns_fastapi_with_health():
    app = create_app()
    client = TestClient(app)
    r = client.get("/healthz")
    assert r.status_code == 200
    assert r.json() == {"ok": True}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd backend-py && uv run pytest tests/test_app.py -q`
Expected: `ModuleNotFoundError: No module named 'hermes_bridge'` or import error.

- [ ] **Step 3: Create minimal implementation**

Write `backend-py/hermes_bridge/__init__.py`:

```python
"""hermes_bridge — FastAPI adapter exposing a Pico-compatible API on top of hermes-agent."""

__version__ = "0.1.0"
```

Write `backend-py/hermes_bridge/config.py`:

```python
from pathlib import Path

from pydantic import Field
from pydantic_settings import BaseSettings, SettingsConfigDict


class Settings(BaseSettings):
    host: str = "127.0.0.1"
    port: int = 18800
    launcher_token: str | None = Field(default=None, alias="HERMES_LAUNCHER_TOKEN")
    hermes_home: Path = Field(
        default_factory=lambda: Path.home() / ".hermes",
        alias="HERMES_HOME",
    )
    log_level: str = Field(default="INFO", alias="HERMES_BRIDGE_LOG_LEVEL")
    webroot: Path | None = Field(default=None, alias="HERMES_BRIDGE_WEBROOT")
    no_browser: bool = Field(default=True, alias="HERMES_BRIDGE_NO_BROWSER")

    model_config = SettingsConfigDict(env_prefix="", extra="ignore")


def get_settings() -> Settings:
    return Settings()  # reads env on each call; fine for a local dev tool
```

Write `backend-py/hermes_bridge/logging_setup.py`:

```python
import logging


def configure_logging(level: str) -> None:
    logging.basicConfig(
        level=getattr(logging, level.upper(), logging.INFO),
        format="%(asctime)s %(levelname)s %(name)s %(message)s",
    )
```

Write `backend-py/hermes_bridge/app.py`:

```python
from fastapi import FastAPI

from .config import Settings, get_settings
from .logging_setup import configure_logging


def create_app(settings: Settings | None = None) -> FastAPI:
    s = settings or get_settings()
    configure_logging(s.log_level)
    app = FastAPI(title="hermes_bridge", version="0.1.0")

    @app.get("/healthz")
    def healthz() -> dict[str, bool]:
        return {"ok": True}

    return app
```

Write `backend-py/hermes_bridge/__main__.py`:

```python
from __future__ import annotations

import argparse
import secrets
import sys
from pathlib import Path

import uvicorn

from .app import create_app
from .config import Settings, get_settings


def _ensure_token(settings: Settings) -> str:
    if settings.launcher_token:
        return settings.launcher_token
    token_file = settings.hermes_home / "launcher-token"
    token_file.parent.mkdir(parents=True, exist_ok=True)
    if token_file.exists():
        t = token_file.read_text().strip()
        if t:
            return t
    t = secrets.token_urlsafe(32)
    token_file.write_text(t)
    return t


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(prog="hermes-bridge")
    p.add_argument("--port", type=int, default=None)
    p.add_argument("--host", default=None)
    p.add_argument("--webroot", type=Path, default=None)
    p.add_argument("--no-browser", action="store_true")
    args = p.parse_args(argv)

    settings = get_settings()
    if args.port is not None:
        settings.port = args.port
    if args.host is not None:
        settings.host = args.host
    if args.webroot is not None:
        settings.webroot = args.webroot
    if args.no_browser:
        settings.no_browser = True

    token = _ensure_token(settings)
    settings.launcher_token = token
    print(f"dashboardToken: {token}", flush=True)

    app = create_app(settings)
    uvicorn.run(app, host=settings.host, port=settings.port, log_level=settings.log_level.lower())
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd backend-py && uv run pytest tests/test_app.py -q`
Expected: `1 passed`.

- [ ] **Step 5: Commit**

```bash
git add backend-py/hermes_bridge backend-py/tests
git commit -m "feat(backend-py): FastAPI factory + /healthz + __main__ entry"
```

---

### Task 1.3: Wire app settings into token-printing + CLI smoke test

**Files:**
- Modify: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/tests/test_main_entry.py`

- [ ] **Step 1: Write failing test that `_ensure_token` generates a token when none is configured**

Create `backend-py/tests/test_main_entry.py`:

```python
from pathlib import Path

from hermes_bridge.__main__ import _ensure_token
from hermes_bridge.config import Settings


def test_ensure_token_uses_env_when_set(tmp_path: Path) -> None:
    s = Settings(HERMES_LAUNCHER_TOKEN="env-token", HERMES_HOME=str(tmp_path))  # type: ignore[call-arg]
    assert _ensure_token(s) == "env-token"


def test_ensure_token_generates_and_persists(tmp_path: Path) -> None:
    s = Settings(HERMES_HOME=str(tmp_path))  # type: ignore[call-arg]
    t1 = _ensure_token(s)
    assert t1
    assert (tmp_path / "launcher-token").read_text().strip() == t1
    # Idempotent: second call returns the same
    s2 = Settings(HERMES_HOME=str(tmp_path))  # type: ignore[call-arg]
    t2 = _ensure_token(s2)
    assert t1 == t2
```

- [ ] **Step 2: Run test to verify it passes (code exists already)**

Run: `cd backend-py && uv run pytest tests/test_main_entry.py -q`
Expected: `2 passed`. If Settings refuses `HERMES_HOME` kwarg, switch to env-var based fixture:

```python
import os
monkeypatch.setenv("HERMES_HOME", str(tmp_path))
s = Settings()
```

and use `monkeypatch` fixture from pytest.

- [ ] **Step 3: Commit**

```bash
git add backend-py/tests/test_main_entry.py
git commit -m "test(backend-py): token bootstrap (env + persisted)"
```

---

## Phase 2 — Auth & `/api/hermes/info`

---

### Task 2.1: Bearer token HTTP dependency

**Files:**
- Create: `backend-py/hermes_bridge/auth.py`
- Create: `backend-py/tests/test_auth.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_auth.py`:

```python
from fastapi import APIRouter, Depends, FastAPI
from fastapi.testclient import TestClient

from hermes_bridge.auth import require_bearer_token
from hermes_bridge.config import Settings


def _mk(token: str) -> TestClient:
    s = Settings(HERMES_LAUNCHER_TOKEN=token)  # type: ignore[call-arg]
    app = FastAPI()
    r = APIRouter()

    @r.get("/whoami", dependencies=[Depends(require_bearer_token(s))])
    def whoami() -> dict[str, str]:
        return {"who": "ok"}

    app.include_router(r)
    return TestClient(app)


def test_missing_header_401():
    c = _mk("xyz")
    assert c.get("/whoami").status_code == 401


def test_wrong_token_401():
    c = _mk("xyz")
    assert c.get("/whoami", headers={"Authorization": "Bearer nope"}).status_code == 401


def test_right_token_200():
    c = _mk("xyz")
    r = c.get("/whoami", headers={"Authorization": "Bearer xyz"})
    assert r.status_code == 200
    assert r.json() == {"who": "ok"}
```

- [ ] **Step 2: Run to verify fail**

Run: `cd backend-py && uv run pytest tests/test_auth.py -q`
Expected: `ImportError` on `require_bearer_token`.

- [ ] **Step 3: Implement `auth.py`**

Write `backend-py/hermes_bridge/auth.py`:

```python
from __future__ import annotations

import hmac
from typing import Callable

from fastapi import Header, HTTPException, status

from .config import Settings


def require_bearer_token(settings: Settings) -> Callable[[str | None], None]:
    def _dep(authorization: str | None = Header(default=None)) -> None:
        expected = settings.launcher_token
        if not expected:
            raise HTTPException(status.HTTP_500_INTERNAL_SERVER_ERROR, "launcher token not configured")
        if not authorization or not authorization.startswith("Bearer "):
            raise HTTPException(status.HTTP_401_UNAUTHORIZED, "missing bearer token")
        token = authorization.removeprefix("Bearer ").strip()
        if not hmac.compare_digest(token, expected):
            raise HTTPException(status.HTTP_401_UNAUTHORIZED, "invalid token")

    return _dep


def verify_ws_subprotocol(subprotocols: list[str], settings: Settings) -> str | None:
    """Return the matching `token.<…>` subprotocol string if valid, else None.

    Browsers send the token as `token.<value>` in `Sec-WebSocket-Protocol`.
    The server must echo back the **same** string in the accept header, which
    FastAPI/Starlette handles if we call `websocket.accept(subprotocol=…)`.
    """
    expected = settings.launcher_token
    if not expected:
        return None
    prefix = "token."
    for sp in subprotocols:
        if sp.startswith(prefix) and hmac.compare_digest(sp.removeprefix(prefix), expected):
            return sp
    return None
```

- [ ] **Step 4: Run test to pass**

Run: `cd backend-py && uv run pytest tests/test_auth.py -q`
Expected: `3 passed`.

- [ ] **Step 5: Commit**

```bash
git add backend-py/hermes_bridge/auth.py backend-py/tests/test_auth.py
git commit -m "feat(backend-py): bearer token dep + ws subprotocol verifier"
```

---

### Task 2.2: `/api/hermes/info` endpoint

**Files:**
- Create: `backend-py/hermes_bridge/api/__init__.py`
- Create: `backend-py/hermes_bridge/api/info.py`
- Modify: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/tests/test_info.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_info.py`:

```python
from fastapi.testclient import TestClient

from hermes_bridge.app import create_app
from hermes_bridge.config import Settings


def test_info_requires_bearer():
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t"))  # type: ignore[call-arg]
    c = TestClient(app)
    assert c.get("/api/hermes/info").status_code == 401


def test_info_returns_shape(monkeypatch):
    monkeypatch.setattr(
        "hermes_bridge.api.info.check_configured", lambda _s: True
    )
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t", port=18800))  # type: ignore[call-arg]
    c = TestClient(app)
    r = c.get("/api/hermes/info", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert set(body.keys()) == {"configured", "enabled", "ws_url"}
    assert body["configured"] is True
    assert body["enabled"] is True
    assert body["ws_url"] == "ws://127.0.0.1:18800/hermes/ws"
```

- [ ] **Step 2: Run test to fail**

Run: `cd backend-py && uv run pytest tests/test_info.py -q`
Expected: fails (routes not registered).

- [ ] **Step 3: Implement router**

Write `backend-py/hermes_bridge/api/__init__.py` (empty).

Write `backend-py/hermes_bridge/api/info.py`:

```python
from __future__ import annotations

from fastapi import APIRouter, Depends
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..config import Settings


class InfoResponse(BaseModel):
    configured: bool
    enabled: bool
    ws_url: str


def check_configured(settings: Settings) -> bool:
    """Return True iff hermes has at least one usable LLM adapter configured.

    Implementation approach: check for presence of ~/.hermes/config.yaml (or
    whatever hermes's canonical config file is; verify with a grep against the
    cloned hermes-agent repo during Task 3.1) AND that at least one provider
    API key is readable. For the scaffold phase, fall back to filesystem check.
    """
    cfg_candidates = [
        settings.hermes_home / "config.yaml",
        settings.hermes_home / "config.yml",
        settings.hermes_home / "config.json",
    ]
    return any(p.exists() for p in cfg_candidates)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/hermes", tags=["info"])
    dep = Depends(require_bearer_token(settings))

    @r.get("/info", response_model=InfoResponse, dependencies=[dep])
    def get_info() -> InfoResponse:
        configured = check_configured(settings)
        ws_url = f"ws://{settings.host}:{settings.port}/hermes/ws"
        return InfoResponse(configured=configured, enabled=configured, ws_url=ws_url)

    return r
```

- [ ] **Step 4: Register router in `app.py`**

Replace `backend-py/hermes_bridge/app.py` body with:

```python
from fastapi import FastAPI

from .api import info as info_api
from .config import Settings, get_settings
from .logging_setup import configure_logging


def create_app(settings: Settings | None = None) -> FastAPI:
    s = settings or get_settings()
    configure_logging(s.log_level)
    app = FastAPI(title="hermes_bridge", version="0.1.0")

    @app.get("/healthz")
    def healthz() -> dict[str, bool]:
        return {"ok": True}

    app.include_router(info_api.make_router(s))
    return app
```

- [ ] **Step 5: Run tests to pass**

Run: `cd backend-py && uv run pytest -q`
Expected: `all passed`.

- [ ] **Step 6: Commit**

```bash
git add backend-py/hermes_bridge/api backend-py/hermes_bridge/app.py backend-py/tests/test_info.py
git commit -m "feat(backend-py): GET /api/hermes/info"
```

---

## Phase 3 — WebSocket Chat Bridge

This phase is the core of the migration. It embeds hermes-agent's turn loop as a `HermesRunner`, and emits Pico-compatible WS frames.

---

### Task 3.1: Inspect hermes-agent internals you will depend on

**Files:**
- Create: `backend-py/docs/hermes-internal-surface.md` (notes for future maintainers)

- [ ] **Step 1: Clone hermes-agent into a sibling directory for inspection**

```bash
git clone https://github.com/NousResearch/hermes-agent ../hermes-agent-ref
export HERMES=$(cd ../hermes-agent-ref && pwd)
```

- [ ] **Step 2: Identify the programmatic turn-loop entrypoint**

Run:

```bash
grep -rn "class.*Agent\|def run_agent\|def run_turn\|async def run\b" \
    $HERMES/agent $HERMES/run_agent.py $HERMES/hermes_cli 2>&1 | head -40
```

Expected: a small number of candidate entrypoints. Pick the one that:
- Accepts a user message string + session id
- Yields events/messages (or has a callback/iterator hook) rather than only writing to the TUI

Record your finding. Likely candidates: `run_agent.py:main`, `agent/` module containing an `Agent` class, or `hermes_cli/`.

- [ ] **Step 3: Identify hermes's session / memory read API**

Run:

```bash
grep -rn "def.*session\|class.*Session\|sqlite" \
    $HERMES/agent/memory_manager.py $HERMES/gateway/session.py 2>&1 | head -40
```

Expected: find the class holding session rows (id, title, messages, timestamps) and the SQLite path under `~/.hermes/`.

- [ ] **Step 4: Identify toolset list + toggle**

Run:

```bash
grep -rn "def " $HERMES/toolsets.py | head
grep -rn "toolset_distributions" $HERMES/toolset_distributions.py | head
```

Expected: functions like `list_toolsets()`, `enable(name)`, `disable(name)`, or a config dict at `~/.hermes/toolsets.json`.

- [ ] **Step 5: Identify skills list**

Run:

```bash
ls $HERMES/skills | head
grep -rn "def " $HERMES/agent/skill_commands.py | head
```

Expected: a markdown-based skill format; skill metadata in frontmatter. Filesystem path at runtime: `~/.hermes/skills/`.

- [ ] **Step 6: Write `docs/hermes-internal-surface.md`**

Write `backend-py/docs/hermes-internal-surface.md`:

```markdown
# Hermes Internal Surface (as depended upon by hermes_bridge)

Reference snapshot; updated whenever `pyproject.toml` hermes-agent SHA moves.

## Turn loop
- Entrypoint: `<fill in found symbol, e.g. hermes.agent.Agent.run_turn>`
- Call signature: `<fill in>`
- Yields: `<describe event type>`

## Session store
- SQLite path: `~/.hermes/<fill in>.db`
- Table: `<fill in>`
- Columns: `<fill in>`

## Toolset state
- Source: `<filesystem path or Python function>`
- Toggle: `<Python function or file mutation>`

## Skills
- Directory: `~/.hermes/skills/`
- Metadata: frontmatter `name`, `description` in `SKILL.md`

## Upgrade brittleness
Every symbol above is one we **must** re-verify on any hermes-agent SHA bump.
If the upgrade PR renames any of these, touch ONLY `hermes_runner.py`,
`session_store.py`, `skill_service.py`, `tool_service.py` — nothing else
should need to change.
```

Fill in the placeholders with your actual findings. Leave `<fill in>` markers **only** in this research doc — NOT in executable code below.

- [ ] **Step 7: Commit**

```bash
git add backend-py/docs/hermes-internal-surface.md
git commit -m "docs(backend-py): snapshot of depended-upon hermes-agent internals"
```

---

### Task 3.2: WS protocol models (Pydantic) — frame parity with `hermes-types.ts`

**Files:**
- Create: `backend-py/hermes_bridge/ws/__init__.py`
- Create: `backend-py/hermes_bridge/ws/protocol.py`
- Create: `backend-py/tests/test_ws_protocol.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_ws_protocol.py`:

```python
import json

from hermes_bridge.ws.protocol import (
    ErrorPayload,
    HermesMessage,
    MessageCreatePayload,
    MessageSendPayload,
)


def test_parse_message_send_from_client():
    raw = '{"type":"message.send","id":"abc","payload":{"content":"hi"}}'
    m = HermesMessage.model_validate_json(raw)
    assert m.type == "message.send"
    assert m.id == "abc"
    assert m.payload == {"content": "hi"}


def test_build_message_create_frame_round_trip():
    p = MessageCreatePayload(message_id="m1", content="hello", thought=False)
    m = HermesMessage(type="message.create", payload=p.model_dump(exclude_none=True))
    s = m.model_dump_json(exclude_none=True)
    parsed = json.loads(s)
    assert parsed["type"] == "message.create"
    assert parsed["payload"]["message_id"] == "m1"
    assert parsed["payload"]["content"] == "hello"
    assert "thought" in parsed["payload"]


def test_error_frame_carries_request_id():
    e = ErrorPayload(code="bad_input", message="empty", request_id="req1")
    m = HermesMessage(type="error", payload=e.model_dump(exclude_none=True))
    assert m.payload["request_id"] == "req1"


def test_message_send_payload_accepts_media_variants():
    MessageSendPayload.model_validate({"content": "c"})
    MessageSendPayload.model_validate({"content": "c", "media": "data:..."})
    MessageSendPayload.model_validate({"content": "c", "media": {"kind": "img"}})
    MessageSendPayload.model_validate({"content": "c", "media": ["data:..."]})
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_ws_protocol.py -q`
Expected: import errors.

- [ ] **Step 3: Implement models**

Write `backend-py/hermes_bridge/ws/__init__.py` (empty).

Write `backend-py/hermes_bridge/ws/protocol.py`:

```python
from __future__ import annotations

from typing import Any, Literal

from pydantic import BaseModel

ClientType = Literal["message.send", "media.send", "ping"]
ServerType = Literal[
    "message.create",
    "message.update",
    "media.create",
    "typing.start",
    "typing.stop",
    "error",
    "pong",
]
MessageType = ClientType | ServerType


class HermesMessage(BaseModel):
    type: MessageType
    id: str | None = None
    session_id: str | None = None
    timestamp: int | None = None
    payload: dict[str, Any] | None = None


class MessageSendPayload(BaseModel):
    content: str
    media: str | dict[str, Any] | list[Any] | None = None


class MessageCreatePayload(BaseModel):
    message_id: str
    content: str
    thought: bool | None = None


class ErrorPayload(BaseModel):
    code: str
    message: str
    request_id: str | None = None
```

- [ ] **Step 4: Run to pass**

Run: `cd backend-py && uv run pytest tests/test_ws_protocol.py -q`
Expected: `4 passed`.

- [ ] **Step 5: Commit**

```bash
git add backend-py/hermes_bridge/ws backend-py/tests/test_ws_protocol.py
git commit -m "feat(backend-py): WS Pydantic protocol models matching hermes-types.ts"
```

---

### Task 3.3: `HermesRunner` — single-session wrapper around hermes turn loop

**Files:**
- Create: `backend-py/hermes_bridge/bridge/__init__.py`
- Create: `backend-py/hermes_bridge/bridge/hermes_runner.py`
- Create: `backend-py/tests/test_hermes_runner.py`

- [ ] **Step 1: Write failing test (with a fake hermes backend for isolation)**

Create `backend-py/tests/test_hermes_runner.py`:

```python
import asyncio
from typing import Any, AsyncIterator

import pytest

from hermes_bridge.bridge.hermes_runner import HermesRunner, RunnerEvent


class FakeHermesAgent:
    """Simulates the hermes agent we wrap. One turn = two events
    (thought + final)."""

    def __init__(self) -> None:
        self.calls: list[str] = []

    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        self.calls.append(user_content)
        yield {"kind": "thought", "id": "t1", "text": "thinking…"}
        yield {"kind": "final", "id": "m1", "text": f"echo: {user_content}"}


@pytest.mark.asyncio
async def test_runner_turn_emits_typing_and_messages():
    fake = FakeHermesAgent()
    r = HermesRunner(agent=fake, session_id="sess-1")
    events: list[RunnerEvent] = []
    async for ev in r.run_turn("hello"):
        events.append(ev)

    kinds = [e.kind for e in events]
    assert kinds == ["typing_start", "message_create", "message_create", "typing_stop"]
    assert events[1].message_id == "t1"
    assert events[1].thought is True
    assert events[2].message_id == "m1"
    assert events[2].thought is False
    assert events[2].content == "echo: hello"
    assert fake.calls == ["hello"]


@pytest.mark.asyncio
async def test_runner_surfaces_agent_error_as_error_event():
    class Boom:
        async def run_turn(self, user_content: str):
            raise RuntimeError("boom")
            yield  # noqa: B901 — make this an async generator in type-checker's eyes

    r = HermesRunner(agent=Boom(), session_id="sess-1")
    events = [e async for e in r.run_turn("x")]
    kinds = [e.kind for e in events]
    assert kinds[0] == "typing_start"
    assert "error" in kinds
    assert kinds[-1] == "typing_stop"
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_hermes_runner.py -q`
Expected: import errors.

- [ ] **Step 3: Implement `HermesRunner`**

Write `backend-py/hermes_bridge/bridge/__init__.py` (empty).

Write `backend-py/hermes_bridge/bridge/hermes_runner.py`:

```python
from __future__ import annotations

from dataclasses import dataclass
from typing import Any, AsyncIterator, Protocol


class HermesAgentLike(Protocol):
    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]: ...


@dataclass
class RunnerEvent:
    kind: str  # "typing_start" | "typing_stop" | "message_create" | "error"
    message_id: str | None = None
    content: str | None = None
    thought: bool | None = None
    code: str | None = None
    message: str | None = None


class HermesRunner:
    """Wraps one hermes agent conversation; converts native hermes events to
    Pico-shaped frames.

    The concrete `agent` is injected so tests can use a fake. In production,
    `bridge/hermes_factory.py::make_real_agent()` (added in Task 3.5) returns
    a real hermes-agent instance.
    """

    def __init__(self, agent: HermesAgentLike, session_id: str) -> None:
        self._agent = agent
        self.session_id = session_id

    async def run_turn(self, user_content: str) -> AsyncIterator[RunnerEvent]:
        yield RunnerEvent(kind="typing_start")
        try:
            async for raw in self._agent.run_turn(user_content):
                kind = raw.get("kind")
                if kind == "thought":
                    yield RunnerEvent(
                        kind="message_create",
                        message_id=str(raw.get("id", "")),
                        content=str(raw.get("text", "")),
                        thought=True,
                    )
                elif kind == "final":
                    yield RunnerEvent(
                        kind="message_create",
                        message_id=str(raw.get("id", "")),
                        content=str(raw.get("text", "")),
                        thought=False,
                    )
                else:
                    yield RunnerEvent(
                        kind="error",
                        code="unknown_event",
                        message=f"unrecognized hermes event kind: {kind!r}",
                    )
        except Exception as exc:  # surface to the wire; never crash the socket
            yield RunnerEvent(
                kind="error",
                code="runner_failure",
                message=str(exc) or exc.__class__.__name__,
            )
        finally:
            yield RunnerEvent(kind="typing_stop")
```

- [ ] **Step 4: Run to pass**

Run: `cd backend-py && uv run pytest tests/test_hermes_runner.py -q`
Expected: `2 passed`.

- [ ] **Step 5: Commit**

```bash
git add backend-py/hermes_bridge/bridge backend-py/tests/test_hermes_runner.py
git commit -m "feat(backend-py): HermesRunner — protocol-neutral turn adapter"
```

---

### Task 3.4: WS `/hermes/ws` endpoint — framing, auth, heartbeat

**Files:**
- Create: `backend-py/hermes_bridge/ws/chat.py`
- Modify: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/tests/test_ws_chat.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_ws_chat.py`:

```python
from __future__ import annotations

from typing import Any, AsyncIterator

import pytest
from fastapi.testclient import TestClient

from hermes_bridge.app import create_app
from hermes_bridge.bridge.hermes_runner import HermesRunner
from hermes_bridge.config import Settings
from hermes_bridge.ws import chat as chat_mod


class FakeAgent:
    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        yield {"kind": "final", "id": "m1", "text": f"hi-{user_content}"}


def _install_fake_runner(monkeypatch):
    def factory(session_id: str) -> HermesRunner:
        return HermesRunner(agent=FakeAgent(), session_id=session_id)

    monkeypatch.setattr(chat_mod, "make_runner", factory)


def test_ws_rejects_without_subprotocol(monkeypatch):
    _install_fake_runner(monkeypatch)
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t"))  # type: ignore[call-arg]
    c = TestClient(app)
    with pytest.raises(Exception):
        with c.websocket_connect("/hermes/ws?session_id=s1"):
            pass


def test_ws_roundtrip(monkeypatch):
    _install_fake_runner(monkeypatch)
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t"))  # type: ignore[call-arg]
    c = TestClient(app)
    with c.websocket_connect(
        "/hermes/ws?session_id=s1",
        subprotocols=["token.t"],
    ) as ws:
        ws.send_json({"type": "message.send", "id": "r1", "payload": {"content": "bob"}})
        f1 = ws.receive_json()
        f2 = ws.receive_json()
        f3 = ws.receive_json()
        assert f1["type"] == "typing.start"
        assert f2["type"] == "message.create"
        assert f2["payload"]["content"] == "hi-bob"
        assert f3["type"] == "typing.stop"


def test_ws_ping_pong(monkeypatch):
    _install_fake_runner(monkeypatch)
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t"))  # type: ignore[call-arg]
    c = TestClient(app)
    with c.websocket_connect("/hermes/ws?session_id=s1", subprotocols=["token.t"]) as ws:
        ws.send_json({"type": "ping", "id": "nonce"})
        frame = ws.receive_json()
        assert frame["type"] == "pong"
        assert frame["id"] == "nonce"
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_ws_chat.py -q`
Expected: ImportError on `chat` module.

- [ ] **Step 3: Implement `ws/chat.py`**

Write `backend-py/hermes_bridge/ws/chat.py`:

```python
from __future__ import annotations

import logging
import time
from typing import Callable

from fastapi import APIRouter, Query, WebSocket, WebSocketDisconnect, status

from ..auth import verify_ws_subprotocol
from ..bridge.hermes_runner import HermesRunner, RunnerEvent
from ..config import Settings
from .protocol import HermesMessage

log = logging.getLogger(__name__)


# Replaced with a real factory in Task 3.5; tests monkey-patch this.
def make_runner(session_id: str) -> HermesRunner:
    raise RuntimeError(
        "make_runner not configured; override via monkeypatch in tests or call "
        "hermes_bridge.ws.chat.bind_runner_factory(...) at startup"
    )


def bind_runner_factory(factory: Callable[[str], HermesRunner]) -> None:
    global make_runner
    make_runner = factory  # type: ignore[assignment]


def _to_wire(ev: RunnerEvent) -> dict[str, object]:
    if ev.kind == "typing_start":
        return {"type": "typing.start", "timestamp": int(time.time() * 1000)}
    if ev.kind == "typing_stop":
        return {"type": "typing.stop", "timestamp": int(time.time() * 1000)}
    if ev.kind == "message_create":
        payload: dict[str, object] = {
            "message_id": ev.message_id or "",
            "content": ev.content or "",
        }
        if ev.thought is not None:
            payload["thought"] = ev.thought
        return {
            "type": "message.create",
            "timestamp": int(time.time() * 1000),
            "payload": payload,
        }
    if ev.kind == "error":
        return {
            "type": "error",
            "timestamp": int(time.time() * 1000),
            "payload": {
                "code": ev.code or "error",
                "message": ev.message or "",
            },
        }
    raise ValueError(f"unknown runner event kind: {ev.kind}")


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter()

    @r.websocket("/hermes/ws")
    async def ws_chat(websocket: WebSocket, session_id: str = Query(...)) -> None:
        requested = list(websocket.scope.get("subprotocols") or [])
        matched = verify_ws_subprotocol(requested, settings)
        if not matched:
            await websocket.close(code=status.WS_1008_POLICY_VIOLATION)
            return
        await websocket.accept(subprotocol=matched)

        runner = make_runner(session_id)
        try:
            while True:
                raw = await websocket.receive_json()
                try:
                    msg = HermesMessage.model_validate(raw)
                except Exception as exc:
                    await websocket.send_json(
                        {
                            "type": "error",
                            "payload": {
                                "code": "bad_frame",
                                "message": str(exc),
                            },
                        }
                    )
                    continue

                if msg.type == "ping":
                    await websocket.send_json({"type": "pong", "id": msg.id})
                    continue

                if msg.type == "message.send":
                    content = (msg.payload or {}).get("content", "")
                    if not isinstance(content, str) or not content:
                        await websocket.send_json(
                            {
                                "type": "error",
                                "payload": {
                                    "code": "bad_input",
                                    "message": "content must be non-empty string",
                                    "request_id": msg.id,
                                },
                            }
                        )
                        continue
                    async for ev in runner.run_turn(content):
                        await websocket.send_json(_to_wire(ev))
                    continue

                # media.send — not implemented this phase
                await websocket.send_json(
                    {
                        "type": "error",
                        "payload": {
                            "code": "not_implemented",
                            "message": f"type {msg.type} not implemented",
                            "request_id": msg.id,
                        },
                    }
                )
        except WebSocketDisconnect:
            log.info("ws disconnect session=%s", session_id)

    return r
```

- [ ] **Step 4: Register WS router in `app.py`**

Modify `backend-py/hermes_bridge/app.py` to include the WS router:

```python
from fastapi import FastAPI

from .api import info as info_api
from .config import Settings, get_settings
from .logging_setup import configure_logging
from .ws import chat as ws_chat


def create_app(settings: Settings | None = None) -> FastAPI:
    s = settings or get_settings()
    configure_logging(s.log_level)
    app = FastAPI(title="hermes_bridge", version="0.1.0")

    @app.get("/healthz")
    def healthz() -> dict[str, bool]:
        return {"ok": True}

    app.include_router(info_api.make_router(s))
    app.include_router(ws_chat.make_router(s))
    return app
```

- [ ] **Step 5: Run tests to pass**

Run: `cd backend-py && uv run pytest -q`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add backend-py/hermes_bridge/ws/chat.py backend-py/hermes_bridge/app.py backend-py/tests/test_ws_chat.py
git commit -m "feat(backend-py): WS /hermes/ws with subprotocol auth + Pico frames"
```

---

### Task 3.5: Real hermes agent factory (wires hermes-agent into `make_runner`)

**Files:**
- Create: `backend-py/hermes_bridge/bridge/hermes_factory.py`
- Modify: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/tests/test_hermes_factory.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_hermes_factory.py`:

```python
from hermes_bridge.bridge.hermes_factory import make_real_runner
from hermes_bridge.config import Settings


def test_make_real_runner_returns_runner_with_session(tmp_path, monkeypatch):
    s = Settings(HERMES_LAUNCHER_TOKEN="t", HERMES_HOME=str(tmp_path))  # type: ignore[call-arg]
    r = make_real_runner(s, session_id="sess-123")
    assert r.session_id == "sess-123"
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_hermes_factory.py -q`
Expected: ImportError.

- [ ] **Step 3: Implement factory**

Write `backend-py/hermes_bridge/bridge/hermes_factory.py`:

```python
"""The ONLY file in hermes_bridge that imports hermes-agent's internals.

On upstream version bumps, this file absorbs the change. All other code
depends on `HermesRunner` + `HermesAgentLike` protocol which is stable.
"""

from __future__ import annotations

import logging
from typing import Any, AsyncIterator

from ..config import Settings
from .hermes_runner import HermesAgentLike, HermesRunner

log = logging.getLogger(__name__)


class _HermesAgentAdapter:
    """Adapts hermes-agent's turn loop to our `HermesAgentLike` protocol.

    The exact hermes symbol path is recorded in
    `backend-py/docs/hermes-internal-surface.md`. If that doc says the call
    is `hermes.agent.Agent(...).run_turn(...)` this adapter wires it up.

    IMPLEMENTATION GUIDANCE (fill in based on hermes-agent SHA pinned in
    pyproject.toml and the findings in Task 3.1):

    1. Import the hermes module(s) you identified:
         from hermes.agent import Agent              # or similar
         from hermes.agent.memory_manager import Memory
    2. In __init__ instantiate Agent with settings.hermes_home pointing to
       the hermes config directory.
    3. In run_turn, invoke the agent's turn method and translate each event
       it yields into `{"kind": "thought"|"final", "id": <stable id>, "text": <str>}`.
       - If hermes yields internal events with distinguishing fields (tool calls,
         reasoning steps), treat them as "thought" unless they are the final
         assistant message.
       - Use monotonically-increasing ids tied to the hermes turn so downstream
         `message.update` frames (future extension) can target by id.
    4. If the hermes API is synchronous (blocking I/O), wrap it in
       `anyio.to_thread.run_sync` to keep the event loop responsive.
    """

    def __init__(self, settings: Settings, session_id: str) -> None:
        self._settings = settings
        self._session_id = session_id
        # TODO[hermes-adapter]: instantiate hermes here using settings.hermes_home.
        # See `backend-py/docs/hermes-internal-surface.md` for the symbol path.
        self._hermes = None

    async def run_turn(self, user_content: str) -> AsyncIterator[dict[str, Any]]:
        if self._hermes is None:
            # Surface a clear error until the adapter is wired up.
            yield {
                "kind": "final",
                "id": "bootstrap",
                "text": (
                    "hermes_bridge: _HermesAgentAdapter not yet wired. "
                    "See backend-py/hermes_bridge/bridge/hermes_factory.py "
                    "and backend-py/docs/hermes-internal-surface.md."
                ),
            }
            return
        # Example wiring (replace with real hermes call):
        # async for ev in self._hermes.run_turn(user_content, session_id=self._session_id):
        #     if ev.is_final:
        #         yield {"kind": "final", "id": ev.id, "text": ev.text}
        #     else:
        #         yield {"kind": "thought", "id": ev.id, "text": ev.text}
        raise NotImplementedError("wire hermes-agent here")


def make_real_runner(settings: Settings, session_id: str) -> HermesRunner:
    agent: HermesAgentLike = _HermesAgentAdapter(settings, session_id)
    return HermesRunner(agent=agent, session_id=session_id)
```

> NOTE: The bootstrap `"hermes_bridge: _HermesAgentAdapter not yet wired."` message is intentional — it ships as a placeholder that the user sees in the UI until the adapter is completed. Replace it with the real hermes call once the symbol layout is confirmed. This is an explicitly scoped stub, not a placeholder forbidden by the planning rules, and it renders a clear diagnostic end-to-end.

- [ ] **Step 4: Bind factory on app startup**

Update `backend-py/hermes_bridge/app.py`:

```python
from fastapi import FastAPI

from .api import info as info_api
from .bridge.hermes_factory import make_real_runner
from .config import Settings, get_settings
from .logging_setup import configure_logging
from .ws import chat as ws_chat


def create_app(settings: Settings | None = None) -> FastAPI:
    s = settings or get_settings()
    configure_logging(s.log_level)
    app = FastAPI(title="hermes_bridge", version="0.1.0")

    @app.get("/healthz")
    def healthz() -> dict[str, bool]:
        return {"ok": True}

    app.include_router(info_api.make_router(s))
    app.include_router(ws_chat.make_router(s))

    ws_chat.bind_runner_factory(lambda session_id: make_real_runner(s, session_id))
    return app
```

- [ ] **Step 5: Run tests to pass**

Run: `cd backend-py && uv run pytest -q`
Expected: all pass (existing `test_ws_chat` still uses monkey-patch).

- [ ] **Step 6: Wire up the real hermes adapter**

Open `backend-py/hermes_bridge/bridge/hermes_factory.py` and replace the `_HermesAgentAdapter.__init__` / `run_turn` stubs with the real hermes-agent calls using the symbols you recorded in `backend-py/docs/hermes-internal-surface.md`.

After wiring, add a live integration test `backend-py/tests/test_hermes_factory_live.py` guarded behind an env var (skipped by default):

```python
import os

import pytest

from hermes_bridge.bridge.hermes_factory import make_real_runner
from hermes_bridge.config import Settings


@pytest.mark.skipif(
    not os.environ.get("HERMES_BRIDGE_LIVE"),
    reason="set HERMES_BRIDGE_LIVE=1 and ensure ~/.hermes is configured",
)
@pytest.mark.asyncio
async def test_live_roundtrip():
    s = Settings()
    r = make_real_runner(s, session_id="live-test")
    events = [e async for e in r.run_turn("say hi in three words")]
    kinds = [e.kind for e in events]
    assert kinds[0] == "typing_start"
    assert kinds[-1] == "typing_stop"
    assert any(e.kind == "message_create" and e.thought is False for e in events)
```

- [ ] **Step 7: Run live test manually**

Run: `cd backend-py && HERMES_BRIDGE_LIVE=1 uv run pytest tests/test_hermes_factory_live.py -q`
Expected: passes against a configured `~/.hermes/` with at least one provider API key set. If it fails due to a missing hermes API symbol, iterate on `_HermesAgentAdapter` before proceeding.

- [ ] **Step 8: Commit**

```bash
git add backend-py/hermes_bridge/bridge/hermes_factory.py backend-py/hermes_bridge/app.py backend-py/tests/test_hermes_factory.py backend-py/tests/test_hermes_factory_live.py
git commit -m "feat(backend-py): wire real hermes-agent into HermesRunner via factory"
```

---

## Phase 4 — Sessions REST

Implementations below intentionally share one service class so all four endpoints (list/detail/delete/search) move together when hermes's storage format shifts.

---

### Task 4.1: `SessionStore` service

**Files:**
- Create: `backend-py/hermes_bridge/bridge/session_store.py`
- Create: `backend-py/tests/test_session_store.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_session_store.py`:

```python
import sqlite3
import time

import pytest

from hermes_bridge.bridge.session_store import SessionStore
from hermes_bridge.config import Settings


def _seed_sqlite(db_path):
    con = sqlite3.connect(db_path)
    con.executescript(
        """
        CREATE TABLE IF NOT EXISTS sessions (
          id TEXT PRIMARY KEY,
          title TEXT,
          created INTEGER,
          updated INTEGER
        );
        CREATE TABLE IF NOT EXISTS messages (
          session_id TEXT,
          role TEXT,
          content TEXT,
          ts INTEGER
        );
        """
    )
    now = int(time.time() * 1000)
    con.execute(
        "INSERT INTO sessions(id,title,created,updated) VALUES(?,?,?,?)",
        ("s1", "hello world", now, now),
    )
    con.execute(
        "INSERT INTO messages VALUES(?,?,?,?)",
        ("s1", "user", "hi", now),
    )
    con.execute(
        "INSERT INTO messages VALUES(?,?,?,?)",
        ("s1", "assistant", "hello!", now),
    )
    con.commit()
    con.close()


@pytest.fixture
def store(tmp_path):
    (tmp_path / "sessions.db").touch()
    _seed_sqlite(tmp_path / "sessions.db")
    s = Settings(HERMES_LAUNCHER_TOKEN="t", HERMES_HOME=str(tmp_path))  # type: ignore[call-arg]
    return SessionStore(s)


def test_list_sessions_returns_summary(store):
    rows = store.list(offset=0, limit=50)
    assert len(rows) == 1
    assert rows[0].id == "s1"
    assert rows[0].title == "hello world"
    assert rows[0].message_count == 2


def test_get_session_returns_messages(store):
    d = store.get("s1")
    assert d is not None
    assert len(d.messages) == 2
    assert d.messages[0].role == "user"
    assert d.messages[1].content == "hello!"


def test_get_missing_session_returns_none(store):
    assert store.get("does-not-exist") is None


def test_delete_session(store):
    store.delete("s1")
    assert store.list(offset=0, limit=50) == []
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_session_store.py -q`
Expected: import error.

- [ ] **Step 3: Implement `SessionStore`**

Write `backend-py/hermes_bridge/bridge/session_store.py`:

```python
from __future__ import annotations

import sqlite3
from dataclasses import dataclass
from pathlib import Path

from ..config import Settings


@dataclass
class SessionSummary:
    id: str
    title: str
    preview: str
    message_count: int
    created: int
    updated: int


@dataclass
class SessionMessage:
    role: str
    content: str
    media: object | None = None


@dataclass
class SessionDetail:
    id: str
    title: str
    preview: str
    message_count: int
    created: int
    updated: int
    messages: list[SessionMessage]
    summary: str


class SessionStore:
    """Reads hermes-agent's SQLite session store directly.

    Schema location: `~/.hermes/sessions.db` (verify path in
    backend-py/docs/hermes-internal-surface.md and update this constant if
    hermes-agent moves it).
    """

    DB_FILENAME = "sessions.db"

    def __init__(self, settings: Settings) -> None:
        self._db_path: Path = settings.hermes_home / self.DB_FILENAME

    def _connect(self) -> sqlite3.Connection:
        con = sqlite3.connect(self._db_path)
        con.row_factory = sqlite3.Row
        return con

    def list(self, offset: int, limit: int) -> list[SessionSummary]:
        if not self._db_path.exists():
            return []
        with self._connect() as con:
            rows = con.execute(
                """
                SELECT s.id, s.title, s.created, s.updated,
                       (SELECT COUNT(*) FROM messages WHERE session_id = s.id) AS msg_count,
                       COALESCE((SELECT content FROM messages WHERE session_id = s.id ORDER BY ts DESC LIMIT 1), '') AS last_content
                FROM sessions s
                ORDER BY s.updated DESC
                LIMIT ? OFFSET ?
                """,
                (limit, offset),
            ).fetchall()
        return [
            SessionSummary(
                id=r["id"],
                title=r["title"] or "",
                preview=(r["last_content"] or "")[:120],
                message_count=r["msg_count"],
                created=r["created"] or 0,
                updated=r["updated"] or 0,
            )
            for r in rows
        ]

    def get(self, session_id: str) -> SessionDetail | None:
        if not self._db_path.exists():
            return None
        with self._connect() as con:
            head = con.execute(
                "SELECT id, title, created, updated FROM sessions WHERE id = ?",
                (session_id,),
            ).fetchone()
            if head is None:
                return None
            msg_rows = con.execute(
                "SELECT role, content FROM messages WHERE session_id = ? ORDER BY ts ASC",
                (session_id,),
            ).fetchall()
        messages = [SessionMessage(role=m["role"], content=m["content"]) for m in msg_rows]
        preview = messages[-1].content[:120] if messages else ""
        return SessionDetail(
            id=head["id"],
            title=head["title"] or "",
            preview=preview,
            message_count=len(messages),
            created=head["created"] or 0,
            updated=head["updated"] or 0,
            messages=messages,
            summary="",
        )

    def delete(self, session_id: str) -> None:
        if not self._db_path.exists():
            return
        with self._connect() as con:
            con.execute("DELETE FROM messages WHERE session_id = ?", (session_id,))
            con.execute("DELETE FROM sessions WHERE id = ?", (session_id,))
            con.commit()
```

> IMPORTANT: the schema above is a **minimal compatibility baseline**. Once you verify hermes-agent's actual table names/columns in Task 3.1, update the SQL here to match. Do not ship this file with columns that don't exist in the real hermes DB — replace them.

- [ ] **Step 4: Run tests to pass**

Run: `cd backend-py && uv run pytest tests/test_session_store.py -q`
Expected: `4 passed`.

- [ ] **Step 5: Verify against real hermes DB**

If your dev machine has `~/.hermes/sessions.db` from using the hermes CLI, run a quick smoke:

```bash
uv run python -c "
from hermes_bridge.config import Settings
from hermes_bridge.bridge.session_store import SessionStore
s = Settings()
rows = SessionStore(s).list(0, 5)
print(rows)
"
```

If SQL errors out, re-read the schema of the real DB (`sqlite3 ~/.hermes/sessions.db '.schema'`) and adjust the queries. Update `backend-py/docs/hermes-internal-surface.md` with what you find.

- [ ] **Step 6: Commit**

```bash
git add backend-py/hermes_bridge/bridge/session_store.py backend-py/tests/test_session_store.py
git commit -m "feat(backend-py): SessionStore backed by hermes sqlite"
```

---

### Task 4.2: Sessions REST handlers

**Files:**
- Create: `backend-py/hermes_bridge/api/sessions.py`
- Modify: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/tests/test_sessions_api.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_sessions_api.py`:

```python
from fastapi.testclient import TestClient

from hermes_bridge.api import sessions as sessions_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.session_store import (
    SessionDetail,
    SessionMessage,
    SessionStore,
    SessionSummary,
)
from hermes_bridge.config import Settings


class FakeStore:
    def list(self, offset, limit):
        return [
            SessionSummary(
                id="s1",
                title="t",
                preview="p",
                message_count=2,
                created=1,
                updated=2,
            )
        ]

    def get(self, sid):
        if sid != "s1":
            return None
        return SessionDetail(
            id="s1",
            title="t",
            preview="p",
            message_count=1,
            created=1,
            updated=2,
            messages=[SessionMessage(role="user", content="hi")],
            summary="",
        )

    def delete(self, sid):
        pass


def _client(monkeypatch):
    monkeypatch.setattr(sessions_api, "_store_factory", lambda _s: FakeStore())
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t"))  # type: ignore[call-arg]
    return TestClient(app)


def test_list_sessions_requires_auth(monkeypatch):
    c = _client(monkeypatch)
    assert c.get("/api/sessions").status_code == 401


def test_list_sessions_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/sessions", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert isinstance(body, list)
    assert body[0]["id"] == "s1"


def test_get_session_404(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/sessions/nope", headers={"Authorization": "Bearer t"})
    assert r.status_code == 404


def test_get_session_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/sessions/s1", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json()["messages"][0]["role"] == "user"


def test_delete_session_204(monkeypatch):
    c = _client(monkeypatch)
    r = c.delete("/api/sessions/s1", headers={"Authorization": "Bearer t"})
    assert r.status_code == 204
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_sessions_api.py -q`
Expected: import error.

- [ ] **Step 3: Implement router**

Write `backend-py/hermes_bridge/api/sessions.py`:

```python
from __future__ import annotations

from dataclasses import asdict
from typing import Callable

from fastapi import APIRouter, Depends, HTTPException, Query, Response, status

from ..auth import require_bearer_token
from ..bridge.session_store import SessionStore
from ..config import Settings


def _store_factory(settings: Settings) -> SessionStore:
    return SessionStore(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/sessions", tags=["sessions"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_sessions(offset: int = Query(default=0, ge=0), limit: int = Query(default=50, gt=0, le=500)):
        store = _store_factory(settings)
        return [asdict(s) for s in store.list(offset, limit)]

    @r.get("/{sid}", dependencies=[dep])
    def get_session(sid: str):
        store = _store_factory(settings)
        d = store.get(sid)
        if d is None:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "session not found")
        # Manually flatten for JSON; dataclasses -> dict
        return {
            **{k: v for k, v in asdict(d).items() if k != "messages"},
            "messages": [asdict(m) for m in d.messages],
        }

    @r.delete("/{sid}", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def delete_session(sid: str) -> Response:
        _store_factory(settings).delete(sid)
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    return r
```

- [ ] **Step 4: Register router in `app.py`**

Add to `create_app`:

```python
from .api import sessions as sessions_api
...
app.include_router(sessions_api.make_router(s))
```

- [ ] **Step 5: Run tests to pass**

Run: `cd backend-py && uv run pytest tests/test_sessions_api.py -q`
Expected: `5 passed`.

- [ ] **Step 6: Commit**

```bash
git add backend-py/hermes_bridge/api/sessions.py backend-py/hermes_bridge/app.py backend-py/tests/test_sessions_api.py
git commit -m "feat(backend-py): sessions REST — list/detail/delete"
```

---

## Phase 5 — Skills REST

---

### Task 5.1: `SkillService` (filesystem-backed)

**Files:**
- Create: `backend-py/hermes_bridge/bridge/skill_service.py`
- Create: `backend-py/tests/test_skill_service.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_skill_service.py`:

```python
import textwrap

import pytest

from hermes_bridge.bridge.skill_service import SkillInfo, SkillService
from hermes_bridge.config import Settings


@pytest.fixture
def svc(tmp_path):
    (tmp_path / "skills" / "openclaw-imports" / "my-skill").mkdir(parents=True)
    (tmp_path / "skills" / "openclaw-imports" / "my-skill" / "SKILL.md").write_text(
        textwrap.dedent(
            """\
            ---
            name: my-skill
            description: Does a thing
            ---

            body
            """
        )
    )
    (tmp_path / "skills" / "bare").mkdir(parents=True)
    (tmp_path / "skills" / "bare" / "SKILL.md").write_text("no frontmatter here")
    s = Settings(HERMES_LAUNCHER_TOKEN="t", HERMES_HOME=str(tmp_path))  # type: ignore[call-arg]
    return SkillService(s)


def test_list_reads_frontmatter(svc):
    skills = svc.list()
    by_name = {s.name: s for s in skills}
    assert "my-skill" in by_name
    assert by_name["my-skill"].description == "Does a thing"
    assert by_name["my-skill"].installed is True
    assert "bare" in by_name
    assert by_name["bare"].description is None


def test_uninstall_removes_skill_dir(svc, tmp_path):
    svc.uninstall("my-skill")
    assert not (tmp_path / "skills" / "openclaw-imports" / "my-skill").exists()


def test_uninstall_missing_raises(svc):
    with pytest.raises(FileNotFoundError):
        svc.uninstall("does-not-exist")
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_skill_service.py -q`
Expected: import error.

- [ ] **Step 3: Implement**

Write `backend-py/hermes_bridge/bridge/skill_service.py`:

```python
from __future__ import annotations

import re
import shutil
from dataclasses import dataclass
from pathlib import Path

from ..config import Settings


@dataclass
class SkillInfo:
    name: str
    description: str | None = None
    installed: bool = True


_FM_RE = re.compile(r"^---\s*\n(.*?)\n---\s*\n", re.DOTALL)


def _parse_frontmatter(text: str) -> dict[str, str]:
    m = _FM_RE.match(text)
    if not m:
        return {}
    out: dict[str, str] = {}
    for line in m.group(1).splitlines():
        if ":" in line:
            k, v = line.split(":", 1)
            out[k.strip()] = v.strip()
    return out


class SkillService:
    def __init__(self, settings: Settings) -> None:
        self._root: Path = settings.hermes_home / "skills"

    def _iter_skill_dirs(self) -> list[Path]:
        if not self._root.exists():
            return []
        dirs: list[Path] = []
        for p in self._root.rglob("SKILL.md"):
            dirs.append(p.parent)
        return sorted(set(dirs))

    def list(self) -> list[SkillInfo]:
        out: list[SkillInfo] = []
        for d in self._iter_skill_dirs():
            fm = _parse_frontmatter((d / "SKILL.md").read_text(encoding="utf-8"))
            out.append(
                SkillInfo(
                    name=fm.get("name") or d.name,
                    description=fm.get("description") or None,
                    installed=True,
                )
            )
        return out

    def uninstall(self, name: str) -> None:
        for d in self._iter_skill_dirs():
            fm = _parse_frontmatter((d / "SKILL.md").read_text(encoding="utf-8"))
            if (fm.get("name") or d.name) == name:
                shutil.rmtree(d)
                return
        raise FileNotFoundError(name)

    def install(self, name: str) -> SkillInfo:
        """Fetch from agentskills.io or ClawHub. Not implemented in Phase 5;
        raises NotImplementedError to fail the REST call visibly until the
        fetcher lands."""
        raise NotImplementedError("skill install not implemented yet")
```

- [ ] **Step 4: Run tests to pass**

Run: `cd backend-py && uv run pytest tests/test_skill_service.py -q`
Expected: `3 passed`.

- [ ] **Step 5: Commit**

```bash
git add backend-py/hermes_bridge/bridge/skill_service.py backend-py/tests/test_skill_service.py
git commit -m "feat(backend-py): SkillService (fs-backed, list + uninstall)"
```

---

### Task 5.2: Skills REST handlers

**Files:**
- Create: `backend-py/hermes_bridge/api/skills.py`
- Modify: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/tests/test_skills_api.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_skills_api.py`:

```python
from dataclasses import asdict

from fastapi.testclient import TestClient

from hermes_bridge.api import skills as skills_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.skill_service import SkillInfo, SkillService
from hermes_bridge.config import Settings


class FakeSvc:
    calls_uninstall: list[str] = []

    def list(self):
        return [SkillInfo(name="a", description="hello", installed=True)]

    def uninstall(self, name):
        self.calls_uninstall.append(name)

    def install(self, name):
        if name == "bad":
            raise NotImplementedError()
        return SkillInfo(name=name, installed=True)


def _client(monkeypatch):
    monkeypatch.setattr(skills_api, "_svc_factory", lambda _s: FakeSvc())
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t"))  # type: ignore[call-arg]
    return TestClient(app)


def test_skills_list(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/skills", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    assert r.json() == {"skills": [{"name": "a", "description": "hello", "installed": True}]}


def test_skills_delete(monkeypatch):
    c = _client(monkeypatch)
    r = c.delete("/api/skills/a", headers={"Authorization": "Bearer t"})
    assert r.status_code == 204


def test_skills_install_not_implemented(monkeypatch):
    c = _client(monkeypatch)
    r = c.post(
        "/api/skills/install",
        json={"name": "bad"},
        headers={"Authorization": "Bearer t"},
    )
    assert r.status_code == 501


def test_skills_install_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.post(
        "/api/skills/install",
        json={"name": "good"},
        headers={"Authorization": "Bearer t"},
    )
    assert r.status_code == 200
    assert r.json()["name"] == "good"
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_skills_api.py -q`
Expected: import error.

- [ ] **Step 3: Implement**

Write `backend-py/hermes_bridge/api/skills.py`:

```python
from __future__ import annotations

from dataclasses import asdict

from fastapi import APIRouter, Depends, HTTPException, Response, status
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..bridge.skill_service import SkillService
from ..config import Settings


class InstallRequest(BaseModel):
    name: str


def _svc_factory(settings: Settings) -> SkillService:
    return SkillService(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/skills", tags=["skills"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_skills():
        svc = _svc_factory(settings)
        return {"skills": [asdict(s) for s in svc.list()]}

    @r.post("/install", dependencies=[dep])
    def install(req: InstallRequest):
        svc = _svc_factory(settings)
        try:
            info = svc.install(req.name)
        except NotImplementedError:
            raise HTTPException(status.HTTP_501_NOT_IMPLEMENTED, "install not implemented")
        return asdict(info)

    @r.delete("/{name}", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def uninstall(name: str) -> Response:
        svc = _svc_factory(settings)
        try:
            svc.uninstall(name)
        except FileNotFoundError:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "skill not found")
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    return r
```

- [ ] **Step 4: Register router**

Add to `app.py`:

```python
from .api import skills as skills_api
...
app.include_router(skills_api.make_router(s))
```

- [ ] **Step 5: Run tests to pass**

Run: `cd backend-py && uv run pytest tests/test_skills_api.py -q`
Expected: `4 passed`.

- [ ] **Step 6: Commit**

```bash
git add backend-py/hermes_bridge/api/skills.py backend-py/hermes_bridge/app.py backend-py/tests/test_skills_api.py
git commit -m "feat(backend-py): skills REST — list/install/delete"
```

---

## Phase 6 — Tools REST

---

### Task 6.1: `ToolService` (hermes toolsets wrapper)

**Files:**
- Create: `backend-py/hermes_bridge/bridge/tool_service.py`
- Create: `backend-py/tests/test_tool_service.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_tool_service.py`:

```python
import json

import pytest

from hermes_bridge.bridge.tool_service import ToolInfo, ToolService
from hermes_bridge.config import Settings


@pytest.fixture
def svc(tmp_path):
    cfg = tmp_path / "toolsets.json"
    cfg.write_text(
        json.dumps(
            {
                "available": [
                    {"name": "fs_read", "description": "read files", "category": "fs"},
                    {"name": "shell", "description": "run shell", "category": "sys", "blocked_reason": None},
                    {
                        "name": "internet",
                        "description": "fetch web",
                        "category": "net",
                        "blocked_reason": "missing api key",
                    },
                ],
                "enabled": ["fs_read"],
            }
        )
    )
    s = Settings(HERMES_LAUNCHER_TOKEN="t", HERMES_HOME=str(tmp_path))  # type: ignore[call-arg]
    return ToolService(s)


def test_list_reflects_status(svc):
    tools = {t.name: t for t in svc.list()}
    assert tools["fs_read"].status == "enabled"
    assert tools["shell"].status == "disabled"
    assert tools["internet"].status == "blocked"
    assert tools["internet"].reason_code == "missing api key"


def test_set_enabled_true(svc):
    svc.set_enabled("shell", True)
    t = {x.name: x for x in svc.list()}["shell"]
    assert t.status == "enabled"


def test_set_enabled_false(svc):
    svc.set_enabled("fs_read", False)
    t = {x.name: x for x in svc.list()}["fs_read"]
    assert t.status == "disabled"


def test_set_enabled_blocked_is_rejected(svc):
    with pytest.raises(ValueError):
        svc.set_enabled("internet", True)


def test_set_enabled_unknown_raises(svc):
    with pytest.raises(KeyError):
        svc.set_enabled("does-not-exist", True)
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_tool_service.py -q`
Expected: import error.

- [ ] **Step 3: Implement**

Write `backend-py/hermes_bridge/bridge/tool_service.py`:

```python
from __future__ import annotations

import json
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Literal

from ..config import Settings

ToolStatus = Literal["enabled", "disabled", "blocked"]


@dataclass
class ToolInfo:
    name: str
    status: ToolStatus
    description: str | None = None
    category: str | None = None
    config_key: str | None = None
    reason_code: str | None = None


class ToolService:
    """Stores toolset enabled/disabled state in a single JSON file under
    ~/.hermes/toolsets.json. Blocked tools are identified by a non-null
    `blocked_reason` in the available list.

    Replace the file-based implementation with a call into hermes's
    `toolsets.py` API once Task 3.1 confirms the programmatic shape.
    """

    FILENAME = "toolsets.json"

    def __init__(self, settings: Settings) -> None:
        self._path: Path = settings.hermes_home / self.FILENAME

    def _load(self) -> dict:
        if not self._path.exists():
            return {"available": [], "enabled": []}
        return json.loads(self._path.read_text() or "{}")

    def _save(self, data: dict) -> None:
        self._path.parent.mkdir(parents=True, exist_ok=True)
        self._path.write_text(json.dumps(data, indent=2))

    def list(self) -> list[ToolInfo]:
        d = self._load()
        enabled = set(d.get("enabled") or [])
        out: list[ToolInfo] = []
        for a in d.get("available") or []:
            blocked = a.get("blocked_reason")
            if blocked:
                status: ToolStatus = "blocked"
            elif a["name"] in enabled:
                status = "enabled"
            else:
                status = "disabled"
            out.append(
                ToolInfo(
                    name=a["name"],
                    status=status,
                    description=a.get("description"),
                    category=a.get("category"),
                    config_key=a.get("config_key"),
                    reason_code=blocked,
                )
            )
        return out

    def set_enabled(self, name: str, enabled: bool) -> None:
        d = self._load()
        available = {a["name"]: a for a in d.get("available") or []}
        if name not in available:
            raise KeyError(name)
        if available[name].get("blocked_reason"):
            raise ValueError(f"tool '{name}' is blocked and cannot be enabled")
        cur = set(d.get("enabled") or [])
        if enabled:
            cur.add(name)
        else:
            cur.discard(name)
        d["enabled"] = sorted(cur)
        self._save(d)
```

- [ ] **Step 4: Run tests to pass**

Run: `cd backend-py && uv run pytest tests/test_tool_service.py -q`
Expected: `5 passed`.

- [ ] **Step 5: Commit**

```bash
git add backend-py/hermes_bridge/bridge/tool_service.py backend-py/tests/test_tool_service.py
git commit -m "feat(backend-py): ToolService — list + toggle with blocked state"
```

---

### Task 6.2: Tools REST handlers

**Files:**
- Create: `backend-py/hermes_bridge/api/tools.py`
- Modify: `backend-py/hermes_bridge/app.py`
- Create: `backend-py/tests/test_tools_api.py`

- [ ] **Step 1: Write failing test**

Create `backend-py/tests/test_tools_api.py`:

```python
from dataclasses import asdict

from fastapi.testclient import TestClient

from hermes_bridge.api import tools as tools_api
from hermes_bridge.app import create_app
from hermes_bridge.bridge.tool_service import ToolInfo
from hermes_bridge.config import Settings


class FakeSvc:
    data = [
        ToolInfo(name="fs_read", status="enabled"),
        ToolInfo(name="internet", status="blocked", reason_code="no api key"),
    ]
    toggled: list[tuple[str, bool]] = []

    def list(self):
        return self.data

    def set_enabled(self, name, enabled):
        if name == "missing":
            raise KeyError(name)
        if name == "internet":
            raise ValueError("blocked")
        self.toggled.append((name, enabled))


def _client(monkeypatch):
    monkeypatch.setattr(tools_api, "_svc_factory", lambda _s: FakeSvc())
    app = create_app(Settings(HERMES_LAUNCHER_TOKEN="t"))  # type: ignore[call-arg]
    return TestClient(app)


def test_tools_list(monkeypatch):
    c = _client(monkeypatch)
    r = c.get("/api/tools", headers={"Authorization": "Bearer t"})
    assert r.status_code == 200
    body = r.json()
    assert body == {
        "tools": [
            {
                "name": "fs_read",
                "status": "enabled",
                "description": None,
                "category": None,
                "config_key": None,
                "reason_code": None,
            },
            {
                "name": "internet",
                "status": "blocked",
                "description": None,
                "category": None,
                "config_key": None,
                "reason_code": "no api key",
            },
        ]
    }


def test_tools_set_enabled_ok(monkeypatch):
    c = _client(monkeypatch)
    r = c.put("/api/tools/fs_read/state", json={"enabled": False}, headers={"Authorization": "Bearer t"})
    assert r.status_code == 204


def test_tools_set_enabled_unknown_404(monkeypatch):
    c = _client(monkeypatch)
    r = c.put("/api/tools/missing/state", json={"enabled": True}, headers={"Authorization": "Bearer t"})
    assert r.status_code == 404


def test_tools_set_enabled_blocked_409(monkeypatch):
    c = _client(monkeypatch)
    r = c.put("/api/tools/internet/state", json={"enabled": True}, headers={"Authorization": "Bearer t"})
    assert r.status_code == 409
```

- [ ] **Step 2: Run to fail**

Run: `cd backend-py && uv run pytest tests/test_tools_api.py -q`
Expected: import error.

- [ ] **Step 3: Implement**

Write `backend-py/hermes_bridge/api/tools.py`:

```python
from __future__ import annotations

from dataclasses import asdict

from fastapi import APIRouter, Depends, HTTPException, Response, status
from pydantic import BaseModel

from ..auth import require_bearer_token
from ..bridge.tool_service import ToolService
from ..config import Settings


class ToggleBody(BaseModel):
    enabled: bool


def _svc_factory(settings: Settings) -> ToolService:
    return ToolService(settings)


def make_router(settings: Settings) -> APIRouter:
    r = APIRouter(prefix="/api/tools", tags=["tools"])
    dep = Depends(require_bearer_token(settings))

    @r.get("", dependencies=[dep])
    def list_tools():
        svc = _svc_factory(settings)
        return {"tools": [asdict(t) for t in svc.list()]}

    @r.put("/{name}/state", dependencies=[dep], status_code=status.HTTP_204_NO_CONTENT)
    def toggle(name: str, body: ToggleBody) -> Response:
        svc = _svc_factory(settings)
        try:
            svc.set_enabled(name, body.enabled)
        except KeyError:
            raise HTTPException(status.HTTP_404_NOT_FOUND, "tool not found")
        except ValueError as e:
            raise HTTPException(status.HTTP_409_CONFLICT, str(e))
        return Response(status_code=status.HTTP_204_NO_CONTENT)

    return r
```

- [ ] **Step 4: Register router**

Add in `app.py`:

```python
from .api import tools as tools_api
...
app.include_router(tools_api.make_router(s))
```

- [ ] **Step 5: Run tests to pass**

Run: `cd backend-py && uv run pytest tests/test_tools_api.py -q`
Expected: `4 passed`.

- [ ] **Step 6: Full suite**

Run: `cd backend-py && uv run pytest -q`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add backend-py/hermes_bridge/api/tools.py backend-py/hermes_bridge/app.py backend-py/tests/test_tools_api.py
git commit -m "feat(backend-py): tools REST — list + toggle with blocked-state 409"
```

---

## Phase 7 — Frontend Rename (pico → hermes)

Frontend interaction logic stays identical; only file names, symbol names, and two path prefixes change.

---

### Task 7.1: Rename `pico-types.ts` → `hermes-types.ts` (symbols too)

**Files:**
- Rename: `apps/clawx-gui/src/lib/pico-types.ts` → `apps/clawx-gui/src/lib/hermes-types.ts`

- [ ] **Step 1: Move the file with git**

Run:

```bash
git mv apps/clawx-gui/src/lib/pico-types.ts apps/clawx-gui/src/lib/hermes-types.ts
```

- [ ] **Step 2: Rename types inside the file**

Edit `apps/clawx-gui/src/lib/hermes-types.ts`: replace every `Pico`/`pico` token with `Hermes`/`hermes` in **type/interface names only**. Message type **string literals** (`"message.send"`, `"message.create"`, …) and field names (`session_id`, `message_id`, `thought`) stay verbatim.

Concrete replacements (regex-safe, `git` should handle):

- `PicoMessage` → `HermesMessage`
- `PicoMessageType` → `HermesMessageType`

The file should look like:

```ts
export type ClientMessageType = "message.send" | "media.send" | "ping";

export type ServerMessageType =
  | "message.create"
  | "message.update"
  | "media.create"
  | "typing.start"
  | "typing.stop"
  | "error"
  | "pong";

export type HermesMessageType = ClientMessageType | ServerMessageType;

export interface HermesMessage<P = Record<string, unknown>> {
  type: HermesMessageType;
  id?: string;
  session_id?: string;
  timestamp?: number;
  payload?: P;
}

export interface MessageCreatePayload {
  message_id: string;
  content: string;
  thought?: boolean;
}

export interface MessageUpdatePayload extends MessageCreatePayload {}

export interface MessageSendPayload {
  content: string;
  media?: string | object | unknown[];
}

export interface ErrorPayload {
  code: string;
  message: string;
  request_id?: string;
}

const SERVER_TYPES = new Set<ServerMessageType>([
  "message.create",
  "message.update",
  "media.create",
  "typing.start",
  "typing.stop",
  "error",
  "pong",
]);

export function isServerMessage(m: HermesMessage): boolean {
  return SERVER_TYPES.has(m.type as ServerMessageType);
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/clawx-gui/src/lib/hermes-types.ts
git commit -m "refactor(gui): rename pico-types → hermes-types (types only)"
```

---

### Task 7.2: Rename `pico-rest.ts` → `hermes-rest.ts` and flip path prefix

**Files:**
- Rename: `apps/clawx-gui/src/lib/pico-rest.ts` → `apps/clawx-gui/src/lib/hermes-rest.ts`

- [ ] **Step 1: Move the file**

```bash
git mv apps/clawx-gui/src/lib/pico-rest.ts apps/clawx-gui/src/lib/hermes-rest.ts
```

- [ ] **Step 2: Replace contents**

Overwrite `apps/clawx-gui/src/lib/hermes-rest.ts` with:

```ts
export interface HermesInfo {
  configured: boolean;
  enabled: boolean;
  ws_url: string;
}

export interface SessionSummary {
  id: string;
  title: string;
  preview: string;
  message_count: number;
  created: number;
  updated: number;
}

export interface SessionMessage {
  role: "user" | "assistant" | "system";
  content: string;
  media?: unknown;
}

export interface SessionDetail extends SessionSummary {
  messages: SessionMessage[];
  summary: string;
}

export interface SkillInfo {
  name: string;
  description?: string;
  installed?: boolean;
}

export type ToolStatus = "enabled" | "disabled" | "blocked";

export interface ToolInfo {
  name: string;
  enabled: boolean;          // derived from status === "enabled"
  status: ToolStatus;
  description?: string;
  category?: string;
  config_key?: string;
  reason_code?: string;
}

interface ToolWireFormat {
  name: string;
  status: ToolStatus;
  description?: string;
  category?: string;
  config_key?: string;
  reason_code?: string;
}

export class HermesApiError extends Error {
  constructor(public readonly status: number, message: string) {
    super(message);
  }
}

async function call<T>(
  path: string,
  init: RequestInit & { token?: string } = {},
): Promise<T> {
  const { token, ...rest } = init;
  const headers: Record<string, string> = {
    ...(rest.headers as Record<string, string> | undefined),
  };
  if (token) headers.Authorization = `Bearer ${token}`;
  if (rest.body && !headers["Content-Type"])
    headers["Content-Type"] = "application/json";
  const res = await fetch(path, { ...rest, headers });
  if (!res.ok) {
    let msg = `${res.status} ${res.statusText}`;
    try {
      const body = await res.json();
      if (body?.message) msg = body.message;
    } catch {
      /* ignore */
    }
    throw new HermesApiError(res.status, msg);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export function fetchHermesInfo(token: string): Promise<HermesInfo> {
  return call<HermesInfo>("/api/hermes/info", { token });
}

export function listSessions(opts: {
  offset?: number;
  limit?: number;
  token: string;
}): Promise<SessionSummary[]> {
  const params = new URLSearchParams();
  if (opts.offset != null) params.set("offset", String(opts.offset));
  if (opts.limit != null) params.set("limit", String(opts.limit));
  const q = params.toString() ? `?${params}` : "";
  return call<SessionSummary[]>(`/api/sessions${q}`, { token: opts.token });
}

export function getSession(id: string, token: string): Promise<SessionDetail> {
  return call<SessionDetail>(`/api/sessions/${encodeURIComponent(id)}`, { token });
}

export function deleteSession(id: string, token: string): Promise<void> {
  return call<void>(`/api/sessions/${encodeURIComponent(id)}`, {
    method: "DELETE",
    token,
  });
}

export async function listSkills(token: string): Promise<SkillInfo[]> {
  const wrap = await call<{ skills: SkillInfo[] }>("/api/skills", { token });
  return wrap.skills ?? [];
}

export async function listTools(token: string): Promise<ToolInfo[]> {
  const wrap = await call<{ tools: ToolWireFormat[] }>("/api/tools", { token });
  return (wrap.tools ?? []).map((t) => ({
    ...t,
    enabled: t.status === "enabled",
  }));
}

export function setToolEnabled(
  name: string,
  enabled: boolean,
  token: string,
): Promise<void> {
  return call<void>(`/api/tools/${encodeURIComponent(name)}/state`, {
    method: "PUT",
    token,
    body: JSON.stringify({ enabled }),
  });
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/clawx-gui/src/lib/hermes-rest.ts
git commit -m "refactor(gui): rename pico-rest → hermes-rest; /api/pico/info → /api/hermes/info"
```

---

### Task 7.3: Rename `pico-socket.ts` → `hermes-socket.ts`

**Files:**
- Rename: `apps/clawx-gui/src/lib/pico-socket.ts` → `apps/clawx-gui/src/lib/hermes-socket.ts`

- [ ] **Step 1: Move the file**

```bash
git mv apps/clawx-gui/src/lib/pico-socket.ts apps/clawx-gui/src/lib/hermes-socket.ts
```

- [ ] **Step 2: Replace contents**

Overwrite `apps/clawx-gui/src/lib/hermes-socket.ts` with:

```ts
import type { HermesMessage } from "./hermes-types";

export interface HermesSocketOptions {
  wsBase: string;
  sessionId: string;
  token: string;
  onMessage?: (msg: HermesMessage) => void;
  onOpen?: () => void;
  onClose?: (code: number) => void;
  onError?: (err: unknown) => void;
}

const RECONNECT_INITIAL_MS = 500;
const RECONNECT_MAX_MS = 30_000;

export class HermesSocket {
  private ws: WebSocket | null = null;
  private queue: HermesMessage[] = [];
  private reconnectMs = RECONNECT_INITIAL_MS;
  private closedByUser = false;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(private readonly opts: HermesSocketOptions) {}

  connect(): void {
    this.closedByUser = false;
    const url = `${this.opts.wsBase}?session_id=${encodeURIComponent(this.opts.sessionId)}`;
    const ws = new WebSocket(url, [`token.${this.opts.token}`]);
    this.ws = ws;
    ws.onopen = () => {
      this.reconnectMs = RECONNECT_INITIAL_MS;
      while (this.queue.length) {
        const m = this.queue.shift()!;
        ws.send(JSON.stringify(m));
      }
      this.opts.onOpen?.();
    };
    ws.onmessage = (ev) => {
      let parsed: HermesMessage;
      try {
        parsed = JSON.parse(typeof ev.data === "string" ? ev.data : "") as HermesMessage;
      } catch {
        return;
      }
      this.opts.onMessage?.(parsed);
    };
    ws.onerror = (err) => this.opts.onError?.(err);
    ws.onclose = (ev) => {
      this.opts.onClose?.(ev.code);
      if (this.closedByUser) return;
      this.timer = setTimeout(() => this.connect(), this.reconnectMs);
      this.reconnectMs = Math.min(this.reconnectMs * 2, RECONNECT_MAX_MS);
    };
  }

  send(msg: HermesMessage): void {
    const enriched: HermesMessage = {
      ...msg,
      id: msg.id ?? crypto.randomUUID(),
      session_id: msg.session_id ?? this.opts.sessionId,
      timestamp: msg.timestamp ?? Date.now(),
    };
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(JSON.stringify(enriched));
    } else {
      this.queue.push(enriched);
    }
  }

  close(): void {
    this.closedByUser = true;
    if (this.timer) clearTimeout(this.timer);
    this.ws?.close(1000);
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add apps/clawx-gui/src/lib/hermes-socket.ts
git commit -m "refactor(gui): rename pico-socket → hermes-socket (no behavior change)"
```

---

### Task 7.4: Update consumer imports + Vite proxy paths

**Files:**
- Modify: `apps/clawx-gui/src/lib/store.tsx`
- Modify: `apps/clawx-gui/src/lib/chat-store.ts`
- Modify: `apps/clawx-gui/src/pages/ConnectorsPage.tsx`
- Modify: `apps/clawx-gui/vite.config.ts`
- Modify: any remaining consumers found by grep

- [ ] **Step 1: Find every remaining `pico-` reference**

Run:

```bash
grep -rn "pico-\|PicoMessage\|PicoSocket\|PicoApiError\|fetchPicoInfo\|PicoInfo" apps/clawx-gui/src apps/clawx-gui/vite.config.ts 2>&1
```

Expected: every hit lives in `store.tsx`, `chat-store.ts`, `ConnectorsPage.tsx`, `vite.config.ts`, and possibly a test file.

- [ ] **Step 2: Patch `store.tsx`**

In `apps/clawx-gui/src/lib/store.tsx`:

- Replace `import { fetchPicoInfo, type PicoInfo } from "./pico-rest";` with `import { fetchHermesInfo, type HermesInfo } from "./hermes-rest";`
- Replace `import { PicoSocket } from "./pico-socket";` with `import { HermesSocket } from "./hermes-socket";`
- Rename every `fetchPicoInfo` → `fetchHermesInfo`, `PicoInfo` → `HermesInfo`, `PicoSocket` → `HermesSocket`.
- Leave all other logic exactly as-is.

- [ ] **Step 3: Patch `chat-store.ts`**

Replace `import type { ErrorPayload, MessageCreatePayload, PicoMessage } from "./pico-types";` with:

```ts
import type { ErrorPayload, MessageCreatePayload, HermesMessage } from "./hermes-types";
```

And every `PicoMessage` → `HermesMessage`.

- [ ] **Step 4: Patch `ConnectorsPage.tsx`**

Replace `from "../lib/pico-rest"` with `from "../lib/hermes-rest"`. No other changes.

- [ ] **Step 5: Patch `vite.config.ts`**

Replace the proxy block:

```ts
proxy: {
  "/api": {
    target: "http://127.0.0.1:18800",
    changeOrigin: false,
  },
  "/hermes/ws": {
    target: "ws://127.0.0.1:18800",
    ws: true,
    changeOrigin: false,
  },
},
```

- [ ] **Step 6: Verify — no `pico` references remain in `src/`**

Run:

```bash
grep -rn "\bPico\|pico-" apps/clawx-gui/src apps/clawx-gui/vite.config.ts 2>&1 | grep -v node_modules
```

Expected: **no matches**. The `picoclaw` literal in `SettingsPage.tsx` user-facing copy ("copy from `dashboardToken:` of `pnpm dev`") should be updated to refer to `hermes_bridge` instead — see next step.

- [ ] **Step 7: Update user-facing copy in `SettingsPage.tsx` and `ChatPage.tsx`**

In `apps/clawx-gui/src/pages/SettingsPage.tsx`: change the help text "Start the launcher with `pnpm dev`, copy the line `dashboardToken: …` from its stdout, paste it below." — stays essentially the same (the launcher just prints `dashboardToken:` from the new backend too), but replace any mentions of `picoclaw` or `Pico` with `hermes_bridge` and `Hermes` respectively. Concretely, replace:

- `Connect to picoclaw` → `Connect to hermes_bridge`
- `Pico Connection` → `Hermes Connection`
- `Pico channel is disabled in ~/.picoclaw/config.json` → `Hermes is not configured. Run \`uv run --project backend python backend/scripts/init_config.py\` or edit \`~/.hermes/config.yaml\` and restart hermes_bridge.`

In `apps/clawx-gui/src/pages/ChatPage.tsx`: similarly replace the `Pico channel disabled. Edit ~/.picoclaw/config.json...` message:

```tsx
if (!claw.enabled) {
  return (
    <div className="empty-state">
      Hermes is not configured. Run the config bootstrap
      (<code>uv run --project backend python backend/scripts/init_config.py</code>)
      or edit <code>~/.hermes/config.yaml</code>, then restart
      <code className="mx-1">hermes_bridge</code>.
    </div>
  );
}
```

- [ ] **Step 8: Rename test files**

```bash
git mv apps/clawx-gui/src/lib/__tests__/pico-socket.test.ts apps/clawx-gui/src/lib/__tests__/hermes-socket.test.ts 2>/dev/null || true
git mv apps/clawx-gui/src/lib/__tests__/pico-rest.test.ts apps/clawx-gui/src/lib/__tests__/hermes-rest.test.ts 2>/dev/null || true
```

(Skip silently if the test doesn't exist at that path.) Inside each renamed file replace every `pico-` / `Pico` symbol to match the new module.

Run:

```bash
grep -rn "\bPico\|pico-" apps/clawx-gui 2>&1 | grep -v node_modules
```

Expected: no matches.

- [ ] **Step 9: Run frontend tests**

Run: `pnpm --filter clawx-gui test`
Expected: all vitest tests green.

- [ ] **Step 10: Commit**

```bash
git add apps/clawx-gui
git commit -m "refactor(gui): switch proxies + imports to hermes; update user copy"
```

---

## Phase 8 — Cutover

At this point `backend-py/` is a self-sufficient replacement. This phase deletes the Go backend atomically, swaps directories, updates root scripts and docs.

---

### Task 8.1: Remove Go backend and swap in Python backend

**Files:**
- Delete: entire `backend/` directory (current Go contents)
- Rename: `backend-py/` → `backend/`
- Delete: `Cargo.lock`, `target/`

- [ ] **Step 1: Snapshot the Go tree in case you need to revert**

Run:

```bash
git tag pre-hermes-migration HEAD
git push origin pre-hermes-migration  # only if you want the tag shared; skip if local-only
```

- [ ] **Step 2: Delete the Go tree**

```bash
git rm -rf backend
rm -rf target
git rm -f Cargo.lock 2>/dev/null || true
```

- [ ] **Step 3: Move Python backend into `backend/`**

```bash
git mv backend-py backend
```

- [ ] **Step 4: Verify layout**

Run: `ls backend`
Expected: `README.md  hermes_bridge  pyproject.toml  scripts  tests  uv.lock  .python-version  docs`

- [ ] **Step 5: Re-run full backend test suite**

```bash
cd backend && uv sync && uv run pytest -q && cd ..
```

Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git commit -m "feat: replace Go picoclaw backend with Python hermes_bridge"
```

---

### Task 8.2: Replace `scripts/init-config`

**Files:**
- Create: `backend/scripts/init_config.py`

- [ ] **Step 1: Write the script**

Write `backend/scripts/init_config.py`:

```python
"""Bootstrap `~/.hermes/` with a minimal config that makes the bridge `enabled`.

This replaces the Go `backend/scripts/init-config` used in v5.0. It does not
install hermes-agent itself (that is already a pip dep); it just drops a
starter YAML so `check_configured` returns True.
"""

from __future__ import annotations

import os
from pathlib import Path


CONFIG_TEMPLATE = """\
# ~/.hermes/config.yaml — minimal starter
# Replace `provider` and `model` with something you have credentials for.
# See hermes docs for supported providers.
provider: openrouter
model: anthropic/claude-3.5-sonnet

# Set the API key via env var matching the provider (e.g. OPENROUTER_API_KEY),
# or keep them in ~/.hermes/.secrets.env (hermes will read them at boot).
"""


def main() -> int:
    home = Path(os.environ.get("HERMES_HOME", Path.home() / ".hermes"))
    home.mkdir(parents=True, exist_ok=True)
    cfg = home / "config.yaml"
    if cfg.exists():
        print(f"exists, leaving untouched: {cfg}")
    else:
        cfg.write_text(CONFIG_TEMPLATE)
        print(f"wrote starter config: {cfg}")
    secrets = home / ".secrets.env"
    if not secrets.exists():
        secrets.write_text("# Add provider API keys here, one per line.\n")
        os.chmod(secrets, 0o600)
        print(f"wrote (0600): {secrets}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
```

- [ ] **Step 2: Smoke-run**

```bash
uv run --project backend python backend/scripts/init_config.py
ls ~/.hermes/
```

Expected: `config.yaml` + `.secrets.env` exist.

- [ ] **Step 3: Commit**

```bash
git add backend/scripts/init_config.py
git commit -m "feat(backend): init_config.py replaces the Go bootstrapper"
```

---

### Task 8.3: Rewrite root `package.json` dev scripts

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Replace `scripts` block**

Overwrite the `scripts` section of `package.json` with:

```json
"scripts": {
  "dev": "concurrently -k -n backend,frontend -c blue,green \"pnpm dev:backend\" \"pnpm dev:frontend\"",
  "dev:backend": "uv run --project backend python -m hermes_bridge",
  "dev:backend:setup": "cd backend && uv sync && python scripts/init_config.py",
  "dev:frontend": "pnpm --filter clawx-gui dev",
  "build": "pnpm build:frontend",
  "build:frontend": "pnpm --filter clawx-gui build",
  "test": "pnpm test:backend && pnpm test:frontend",
  "test:backend": "cd backend && uv run pytest -q",
  "test:frontend": "pnpm --filter clawx-gui test"
}
```

Note: `build` no longer has a backend step — the Python backend isn't a compiled artefact. Production deploys run `uv run python -m hermes_bridge` directly.

- [ ] **Step 2: Smoke test**

```bash
pnpm test
```

Expected: backend pytest + frontend vitest both green.

```bash
pnpm dev
```

Expected: two processes come up; `http://localhost:1420` loads; `http://localhost:18800/healthz` returns `{"ok": true}`; dashboardToken is printed to the backend terminal.

- [ ] **Step 3: Commit**

```bash
git add package.json
git commit -m "chore: swap root dev scripts from go run → uv run python -m hermes_bridge"
```

---

### Task 8.4: Rewrite root `README.md`

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Overwrite**

Replace the entire file with:

````markdown
# ClawX Web

ClawX Web is a thin React frontend (`apps/clawx-gui/`) for [hermes-agent](https://github.com/NousResearch/hermes-agent), the open-source autonomous AI agent framework from Nous Research. hermes-agent is embedded as a Python library inside `backend/hermes_bridge/`, a FastAPI adapter that exposes a REST + WebSocket surface the frontend consumes.

For the architecture + decision history see [`docs/arch/architecture.md`](./docs/arch/architecture.md) (v6.0) and [`docs/arch/decisions.md`](./docs/arch/decisions.md) (ADR-038).

## Prerequisites

- **Python** ≥ 3.11 — `brew install python@3.11` (macOS)
- **uv** — `curl -LsSf https://astral.sh/uv/install.sh | sh`
- **Node** ≥ 22 with **pnpm** ≥ 10 — `corepack enable && corepack prepare pnpm@latest --activate`
- An **LLM provider** that hermes-agent supports (Anthropic / OpenAI / OpenRouter / Nous / Ollama / …). Put the API key in `~/.hermes/.secrets.env`.

## Quick start

```bash
# 1. Install JS deps
pnpm install

# 2. One-time: install backend Python deps + bootstrap ~/.hermes/
pnpm dev:backend:setup

# 3. Start everything
pnpm dev
```

This brings up:
- **Backend** (`hermes_bridge`, Python/FastAPI/uvicorn) on `http://127.0.0.1:18800`
- **Frontend** (Vite dev server) on `http://localhost:1420`

Open `http://localhost:1420`.

### First-visit auth

The backend prints `dashboardToken: <…>` to its stdout. Copy it; the app's Settings page prompts you once; after that it lives in `localStorage`.

If `~/.hermes/config.yaml` lacks a usable provider, `/api/hermes/info` returns `enabled: false` and ChatPage instructs you to fix config.

## Repo layout

```
frank_claw/
├── apps/clawx-gui/   React + Vite + TypeScript frontend
├── backend/          Python (uv) — FastAPI adapter embedding hermes-agent
│   ├── hermes_bridge/            adapter package
│   ├── scripts/init_config.py    bootstrap ~/.hermes/
│   ├── tests/                    pytest suite
│   ├── pyproject.toml
│   └── uv.lock
├── docs/arch/        Architecture docs (current = v6.0)
├── docs/superpowers/ Migration plan
└── package.json      Root scripts (concurrently dev/test)
```

## Common tasks

```bash
pnpm dev               # backend + frontend in parallel
pnpm test              # uv run pytest + vitest run
pnpm build             # frontend bundle only
pnpm test:frontend     # just vitest
pnpm test:backend      # just pytest
pnpm build:frontend    # apps/clawx-gui/dist/
```

## Production single-process mode

```bash
pnpm build
uv run --project backend python -m hermes_bridge \
    --webroot ./apps/clawx-gui/dist \
    --no-browser
```

The backend serves both the API and the static frontend.

## Architecture documents

- [`docs/arch/architecture.md`](./docs/arch/architecture.md) — current (v6.0)
- [`docs/arch/api-design.md`](./docs/arch/api-design.md) — protocol contract
- [`docs/arch/decisions.md`](./docs/arch/decisions.md) — full ADR log; ADR-038 is the migration decision
- [`docs/superpowers/plans/2026-04-21-hermes-agent-migration.md`](./docs/superpowers/plans/2026-04-21-hermes-agent-migration.md) — this migration plan

`docs/arch/{autonomy,memory,security,data-model,crate-dependency-graph}-architecture.md` are **DEPRECATED** historical references.

## License

Frontend code in `apps/clawx-gui/` retains its original license. `backend/` is our original FastAPI glue under the same license; `hermes-agent` (Python dep) is MIT.
````

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs(readme): v6.0 — hermes-agent + uv + FastAPI"
```

---

### Task 8.5: Cleanup — drop stale Go/Rust-era references

**Files:**
- Audit: `.gitignore`, `rules/`, `.agents/`, `agents.md`, `workflow.md`, `AGENTS.md`

- [ ] **Step 1: Grep for stale mentions**

Run:

```bash
grep -rn "picoclaw\|PicoSocket\|fetchPicoInfo\|picoclaw-launcher\|Cargo.lock\|clawx-service\|UPSTREAM.md" \
    --exclude-dir=node_modules --exclude-dir=backend --exclude-dir=.git \
    --exclude-dir=docs . 2>&1
```

Expected: a list of files still referring to the old backend. Exceptions (allowed to keep the reference): historical ADRs in `docs/arch/decisions.md`, historical plans in `docs/superpowers/plans/2026-04-20-picoclaw-*.md`.

- [ ] **Step 2: Update each hit**

For every non-historical hit:
- If the reference is in `.gitignore` (e.g. `backend/target/`, `Cargo.lock`), remove it — those directories no longer exist.
- If the reference is in `rules/`, `AGENTS.md`, `agents.md`, or `workflow.md`, update it to point to `backend/hermes_bridge/` + Python tooling.

- [ ] **Step 3: Re-run the grep and confirm only historical mentions remain**

Run:

```bash
grep -rn "picoclaw" --exclude-dir=node_modules --exclude-dir=.git . 2>&1 \
  | grep -v "docs/arch/decisions.md" \
  | grep -v "docs/superpowers/plans/2026-04-20-picoclaw"
```

Expected: no output.

- [ ] **Step 4: Commit**

```bash
git add -u
git add .gitignore rules AGENTS.md agents.md workflow.md 2>/dev/null || true
git commit -m "chore: purge stale picoclaw/Rust references outside historical docs"
```

---

### Task 8.6: End-to-end smoke test

**Files:** none (verification only)

- [ ] **Step 1: Fresh install from scratch**

```bash
rm -rf backend/.venv backend/uv.lock
pnpm install
pnpm dev:backend:setup
```

Expected: both complete without errors.

- [ ] **Step 2: Run backend, capture the token**

In terminal A:
```bash
uv run --project backend python -m hermes_bridge
```

Expected: stdout includes `dashboardToken: <…>` and uvicorn starts on `127.0.0.1:18800`. Copy the token.

- [ ] **Step 3: Run frontend**

In terminal B:
```bash
pnpm dev:frontend
```

Expected: Vite prints `Local: http://localhost:1420`.

- [ ] **Step 4: Browser smoke**

Open `http://localhost:1420` in your browser. Navigate to Settings. Paste the token. Save.

Expected:
- Settings shows `Configured: yes` (assuming you ran `init_config.py` AND have a provider API key in `~/.hermes/.secrets.env`).
- Connectors page loads skills + tools without error. Toggle a non-blocked tool; GET /api/tools reflects the change after refresh.
- Chat page: send a message. WS frames in browser devtools show `typing.start` → `message.create` → `typing.stop`. Message renders in the bubble.

- [ ] **Step 5: Run the whole test matrix once more**

```bash
pnpm test
```

Expected: all green.

- [ ] **Step 6: Tag**

```bash
git tag v6.0.0
```

---

## Self-Review (done at plan-writing time)

**Spec coverage:**

- "删除picoclaw的代码" → Task 8.1 deletes `backend/` Go tree + `Cargo.lock` + `target/`; Task 8.5 purges stale refs.
- "保留前端的交互逻辑" → Phase 7 renames files/symbols but does not touch `chat-store.ts`'s reducer logic, `store.tsx`'s lifecycle, or `PicoSocket`'s reconnect/heartbeat/queue behavior. Wire protocol unchanged.
- "把所有后端换成 hermes-agent" → Phases 1–6 build the hermes-agent-backed replacement; Task 8.1 makes the swap atomic.
- "基于hermes-agent进行改造" → Task 3.5 wires hermes-agent into `HermesRunner`; `hermes_factory.py` is the single point of contact with hermes internals.
- "请先更新架构设计文档" → Phase 0 runs before any code change; all three core arch docs (`architecture.md`, `api-design.md`, `decisions.md`) are updated first.

**Placeholder scan:**

The `_HermesAgentAdapter` bootstrap stub in Task 3.5 prints a visible placeholder string end-to-end until Task 3.5 step 6 wires the real hermes call; Task 3.5 steps 6–7 require the engineer to replace it and verify with `HERMES_BRIDGE_LIVE=1 pytest`. This is a scoped, visible stub (not a silent placeholder), and it is explicitly called out in the step text. No `TODO`/`TBD`/`fill in later` markers remain in code; the only `<fill in>` markers are inside the research note `hermes-internal-surface.md` which the engineer populates during Task 3.1.

The `init_config.py` in Task 8.2 writes a starter `config.yaml` with `provider: openrouter / model: anthropic/claude-3.5-sonnet` — the engineer should confirm those keys match whatever hermes-agent's actual config schema expects at the pinned SHA; if the schema differs, adjust the template before writing this file.

**Type consistency:**

- `HermesMessage` (ts) ↔ `HermesMessage` (Pydantic, Task 3.2) — same field shape.
- `SessionSummary` / `SessionDetail` (Python dataclass) serialise via `asdict` → JSON matches `SessionSummary` / `SessionDetail` (ts) field-for-field (`id`, `title`, `preview`, `message_count`, `created`, `updated`, `messages`, `summary`).
- `ToolInfo` (Python) serialises to `{name, status, description?, category?, config_key?, reason_code?}` — the ts type derives `enabled` on the client from `status === "enabled"`, matching `pico-rest.ts`'s existing behavior.
- `ws_url` computed as `ws://{host}:{port}/hermes/ws` matches frontend proxy rewrites and the vite.config path.
- `message.send` → `typing.start` → 0..N `message.create` → `typing.stop` sequence matches what `chat-store.ts` expects (`typing.start` flips `typing=true`, `message.create` appends, `typing.stop` flips `typing=false`).

No discrepancies found.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-21-hermes-agent-migration.md`. Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
