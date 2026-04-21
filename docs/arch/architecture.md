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

v4.2 Rust-era 和 v5.0 picoclaw-era 的详细架构文档已清理；如需追溯见 git history。
