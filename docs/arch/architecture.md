# ClawX Architecture v5.0

**日期:** 2026-04-21 

> ClawX 当前由 vendored 的 [sipeed/picoclaw](https://github.com/sipeed/picoclaw) Go 后端与 `apps/clawx-gui/` 中的 React Web 前端组成。本文只描述当前仍然存在的架构与能力边界，不再保留迁移过程中的删改清单。

---

## 1. 架构总览

```
┌──────────────────────────────────────────────────────────────────────┐
│ Browser (any modern browser)                                         │
│   ClawX Web UI (React + Vite + TypeScript)                            │
│     ├── ChatPage                                                      │
│     ├── ConnectorsPage   (skills + tools 浏览)                        │
│     └── SettingsPage     (dashboard token paste + Pico status)       │
└─────────┬────────────────────────────────────┬───────────────────────┘
          │ Vite dev server (1420) proxies     │ Vite dev proxies WS
          │ /api/* → 18800                     │ /pico/ws → 18800
          ▼                                    ▼
┌──────────────────────────────────────────────────────────────────────┐
│ picoclaw-launcher  (Go binary built from backend/web/backend/)       │
│   :18800  the only HTTP entry — REST /api/* + WS /pico/ws (proxied   │
│           into the gateway child process) + bundled web console      │
│           (legacy; we serve our own dist/ via -webroot in prod)      │
│   :18790  picoclaw gateway (CHILD process; auto-spawns only when an  │
│           LLM provider is configured). Frontend NEVER hits this      │
│           directly — all traffic goes through the launcher on 18800. │
│                                                                      │
│   内部能力（在 backend/pkg/* 内可读可改）：                            │
│   • 30+ LLM providers (Anthropic / OpenAI / Ollama …)                 │
│   • Tool-use loop                                                     │
│   • MCP client (stdio / SSE / HTTP)                                   │
│   • Skills (markdown skill files via ClawHub registry)                │
│   • Hooks / SubTurn / Steering                                        │
│   • Channels: Pico, Pico-Client, Telegram, Discord, …                 │
│   • Cron scheduler                                                    │
└──────────────────────────────────────────────────────────────────────┘
```

**单一仓库，两个子项目**：`apps/clawx-gui/`（前端）+ `backend/`（picoclaw vendored）。

---

## 2. 仓库布局（v5.0）

```
frank_claw/
├── apps/
│   └── clawx-gui/              # 前端（React/Vite/TS）
│       ├── src/
│       │   ├── pages/
│       │   │   ├── ChatPage.tsx
│       │   │   ├── ConnectorsPage.tsx
│       │   │   └── SettingsPage.tsx
│       │   ├── components/     # 仅保留 chat 相关
│       │   ├── lib/
│       │   │   ├── pico-rest.ts   # REST 封装
│       │   │   ├── pico-socket.ts # WS 封装
│       │   │   ├── chat-store.ts  # message-id merge store
│       │   │   ├── pico-types.ts  # 协议类型
│       │   │   └── store.tsx      # React context
│       │   └── ...
│       ├── package.json
│       └── vite.config.ts      # port 1420; proxy /api/*, /pico/ws → :18800
├── backend/                    # picoclaw 源码 vendor
│   ├── cmd/
│   │   ├── picoclaw/           # CLI: agent / gateway / onboard / skills / …
│   │   ├── picoclaw-launcher-tui/  # 上游 TUI launcher（我们不用）
│   │   └── membench/           # 上游基准测试工具（我们不用）
│   ├── web/
│   │   ├── backend/            # ← 我们用的 launcher 二进制 main.go 在此
│   │   └── frontend/           # 上游自带前端，**保留**（编译时被 embed）
│   ├── pkg/                    # 各 channel / tool / provider 实现
│   ├── scripts/init-config/    # 本地 patch：替代 broken `picoclaw onboard`
│   ├── internal/
│   ├── go.mod / go.sum
│   ├── Makefile                # `make build-launcher` orchestrates 全套
│   ├── LICENSE                 # 上游 MIT，原样保留
│   ├── UPSTREAM.md             # 记录基线 SHA + 同步流程
│   └── PATCHES.md              # 记录我们对上游做过的所有本地改动
├── docs/
│   ├── arch/                   # v5.0 文档
│   ├── prd/                    # 
│   └── superpowers/plans/2026-04-20-picoclaw-migration.md
├── package.json                # 根 workspace（可选 concurrently 跑前后端）
├── README.md
└── AGENTS.md
```

---

## 3. 协议层

详见 [api-design.md](./api-design.md)。摘要：

### 3.1 WebSocket（聊天主通道）

| 项 | 值 |
|---|---|
| URL（dev） | `ws://localhost:1420/pico/ws?session_id=<uuid>`（经 Vite proxy） |
| URL（直连） | `ws://127.0.0.1:18800/pico/ws?session_id=<uuid>` |
| 鉴权 | `Sec-WebSocket-Protocol: token.<dashboardToken>`（**本仓库 PATCH**，见 `backend/PATCHES.md` 2026-04-20 条目） |
| Token 来源 | 用户从 launcher 终端复制 `dashboardToken: …` 粘到 SettingsPage（持久化到 `localStorage["clawx.dashboard_token"]`）。然后 `GET /api/pico/info` 验证 + 拿 ws_url |
| 消息封套 | `{ type, id?, session_id?, timestamp?, payload? }` |
| 客户端→服务端 | `message.send` / `media.send` / `ping` |
| 服务端→客户端 | `message.create` / `message.update` / `media.create` / `typing.start` / `typing.stop` / `error` / `pong` |
| 流式策略 | **无 token 级别 streaming**；服务端每条消息完成后下发一条 `message.create`；中间过程用 `payload.thought:true` |
| 消息合并 | 同一 `payload.message_id` 的 `message.create` + `message.update` 合并渲染 |

### 3.2 REST（管理面）

所有 `/api/*` 请求带 `Authorization: Bearer <dashboardToken>`。

| 用途 | 端点 |
|---|---|
| 验证 token + 拿 ws_url | `GET /api/pico/info` → `{configured, enabled, ws_url}` |
| 会话 列表 / 详情 / 删除 | `GET /api/sessions[?offset&limit]` / `GET /api/sessions/:id` / `DELETE /api/sessions/:id` |
| Skills 浏览 / 安装 / 卸载 | `GET /api/skills` (返回 `{skills: [...]}`) / `POST /api/skills/install` / `DELETE /api/skills/:name` |
| Tools 列出 / 启停 | `GET /api/tools` (返回 `{tools: [...]}`，每项 `status: "enabled"\|"disabled"\|"blocked"`) / `PUT /api/tools/:name/state` body `{enabled: bool}` |

> 端点已在 Phase 1.4 经 `curl` 实测（见 `docs/superpowers/plans/2026-04-20-picoclaw-surface-audit.md`）。WS 子协议鉴权 patch 在 Phase 2.1（见 `backend/PATCHES.md`）。

---

## 4. 当前前端职责边界

| 区域 | 当前能力 |
|---|---|
| ChatPage | 基于 Pico WebSocket 建立会话、发送消息、接收 `message.create` / `message.update`、显示 typing / thought 状态 |
| ConnectorsPage | 列出已安装 skills、列出 tools、切换 tool 启停状态 |
| SettingsPage | 粘贴 / 清除 dashboard token、刷新 Pico 连接信息、展示 `configured` / `enabled` / `ws_url` |
| 本地配置 | provider、channel、安全配置由 picoclaw 本地文件负责，主要是 `~/.picoclaw/config.json` 与 `~/.picoclaw/.security.yml` |
| 非前端管理面 | 其他 channels、MCP、hooks、cron、skills registry 等能力由 `backend/` 内的 picoclaw 进程直接承担，当前前端不额外封装独立管理界面 |

---

## 5. 部署形态

### 5.1 开发模式

需要：Go ≥ 1.25、Node ≥ 22 (pnpm)、本地 Ollama（或任意 picoclaw 支持的 provider）。

```bash
# 一次性：编辑 ~/.picoclaw/config.json + .security.yml 配好至少一个 provider
cd backend && go run ./scripts/init-config && cd ..   # 首次生成默认 config（替代上游 broken onboard）

# 一次性：构建 launcher 嵌入的前端（go embed.FS 编译期依赖）
pnpm dev:backend:setup

# 日常：根目录一条命令并行起前后端
pnpm dev    # 内部 concurrently 跑：
            #   1) go run ./backend/web/backend -console -no-browser  → launcher :18800
            #   2) pnpm --filter clawx-gui dev                         → vite :1420 (proxy /api/* 与 /pico/ws)
```

打开浏览器到 `http://localhost:1420`，从 launcher 终端复制 `dashboardToken: …` 粘到 SettingsPage。

### 5.2 生产 / 单机模式

```bash
pnpm build       # 产出：backend/build/picoclaw-launcher  +  apps/clawx-gui/dist/
./backend/build/picoclaw-launcher -webroot ./apps/clawx-gui/dist -no-browser
```

launcher 同时托管前端静态资产 + 后端 API。无需 docker，无需 nginx。

---

## 6. 技术栈

| 层 | 技术 | 备注 |
|---|---|---|
| 前端 UI | React 19 + TypeScript 5 | `apps/clawx-gui/` 主体实现 |
| 前端构建 | Vite 6 | dev server 监听 `1420`，代理 `/api` 与 `/pico/ws` |
| 路由 | react-router-dom 7 | `ChatPage` / `ConnectorsPage` / `SettingsPage` |
| Markdown | react-markdown + remark-gfm + highlight.js | 消息渲染 |
| 前端测试 | vitest 4 + @testing-library/react + jsdom | 组件与协议适配测试 |
| 后端语言 | Go ≥ 1.25 | vendored picoclaw |
| 后端测试 | `go test ./...` | 以后端仓库自带 Go tests 为基线 |
| 进程管理 | concurrently（dev）；裸二进制（prod） | 本地单机运行，无额外反向代理要求 |

---

## 7. 安全模型

picoclaw（Go 进程）独占安全责任，配置在 `backend/`。前端约束：

1. WS / REST 全部走 loopback (`127.0.0.1:18800`) 或受信内网
2. Pico token 只在 `localStorage["clawx.dashboard_token"]` 暂存，不进 cookie，不回传任何外部域；UI 中以 `XXXX…XXXX` 掩码展示
3. 前端不直接持有 LLM API Key —— 全部托管在 `~/.picoclaw/.security.yml`
4. 前端不实现任何沙箱 / 命令执行；所有 tool 调用在后端完成

---

## 8. 上游同步策略

picoclaw 主分支演进极快（每天数 commit）。本仓库的策略：

1. **基线 SHA** 写在 `backend/UPSTREAM.md`，是当前 vendor 的源 commit
2. 不自动同步。每次决定同步时：
   - 在新分支用 `scripts/sync-upstream.sh <sha>` 拉取上游 diff
   - 三方合并到 `backend/`，解决冲突时优先保留 `backend/PATCHES.md` 标记的本地改动
   - 升级后跑 `go test ./backend/...` + 前端 `pnpm vitest run`，全绿才合并
3. **永不**通过 `git pull origin main` 直接合上游历史 —— 我们维持自己的 commit graph

---

## 9. 历史文档

以下 v4.2 文档保留为历史参考，**与 v5.0 不再一致**：

- [autonomy-architecture.md](./autonomy-architecture.md) — DEPRECATED
- [memory-architecture.md](./memory-architecture.md) — DEPRECATED
- [security-architecture.md](./security-architecture.md) — DEPRECATED
- [data-model.md](./data-model.md) — DEPRECATED
- [crate-dependency-graph.md](./crate-dependency-graph.md) — DEPRECATED
