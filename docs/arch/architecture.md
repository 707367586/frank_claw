# ClawX Architecture v5.0

**日期:** 2026-04-20 | **对应 ADR:** [ADR-037 (v2)](./decisions.md#adr-037-2026-04-20-删除-rust-后端将-picoclaw-源码-vendor-进本仓库作为新后端) | **取代:** v4.2

> **重大变更**：自 v5.0 起，ClawX 不再持有自研 Rust 后端。后端来自 [sipeed/picoclaw](https://github.com/sipeed/picoclaw) 主分支源码，**vendor 进本仓库的 `backend/` 目录**，由我们自由维护、可自由修改。前端 `apps/clawx-gui/` 是它的纯 Web 客户端。

---

## 1. 架构总览

```
┌──────────────────────────────────────────────────────────────────────┐
│ Browser (any modern browser)                                         │
│   ClawX Web UI (React + Vite + TypeScript)                            │
│     ├── ChatPage                                                      │
│     ├── ConnectorsPage   (skills + tools 浏览)                        │
│     └── SettingsPage     (Pico token + gateway URL)                   │
└─────────┬────────────────────────────────────┬───────────────────────┘
          │ Vite dev server (5173) proxies     │ Vite dev proxies WS
          │ /api/* → 18790                     │ /pico/ws → 18790
          ▼                                    ▼
┌──────────────────────────────────────────────────────────────────────┐
│ picoclaw-launcher  (Go binary built from this repo's backend/)       │
│   :18790  HTTP gateway + Pico WS                                      │
│   :18800  legacy bundled Web Console (we don't use; can serve our    │
│           dist/ instead via -webroot flag in production)             │
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
│       ├── package.json        # 已移除 @tauri-apps/cli
│       └── vite.config.ts      # proxy /api/*, /pico/ws → :18790
├── backend/                    # picoclaw 源码 vendor
│   ├── cmd/
│   │   ├── picoclaw/           # gateway-only 二进制（CLI 用）
│   │   └── picoclaw-launcher/  # 我们的主用二进制（带 /api/*）
│   ├── pkg/                    # 各 channel / tool / provider 实现
│   ├── internal/
│   ├── web/frontend/           # 上游自带前端，**保留但不使用**（编译时被 embed）
│   ├── go.mod / go.sum
│   ├── Makefile
│   ├── LICENSE                 # 上游 MIT，原样保留
│   ├── UPSTREAM.md             # 记录基线 SHA + 同步流程
│   └── PATCHES.md              # 记录我们对上游做过的所有本地改动
├── docs/
│   ├── arch/                   # v5.0 文档
│   ├── prd/                    # 待重写（DEPRECATED）
│   └── superpowers/plans/2026-04-20-picoclaw-migration.md
├── package.json                # 根 workspace（可选 concurrently 跑前后端）
├── README.md
└── AGENTS.md
```

**已移除（migration 完成后）**：`crates/`、`apps/clawx-service/`、`apps/clawx-cli/`、`apps/clawx-gui/src-tauri/`、`Cargo.{toml,lock}`、`rust-toolchain.toml`、`clippy.toml`、`rustfmt.toml`、`target/`。

**已撤回（迁移过程中曾尝试，已废弃）**：`docker-compose.yml`、`Dockerfile` —— 不再用 docker。

---

## 3. 协议层

详见 [api-design.md](./api-design.md)。摘要：

### 3.1 WebSocket（聊天主通道）

| 项 | 值 |
|---|---|
| URL | `ws://localhost:5173/pico/ws?session_id=<uuid>`（dev，经 Vite proxy） |
| 真实后端 | `ws://127.0.0.1:18790/pico/ws?session_id=<uuid>` |
| 鉴权 | Sec-WebSocket-Protocol: `token.<value>` |
| Token 来源 | `GET /api/pico/token` → `{ token, ws_url, enabled }` |
| 消息封套 | `{ type, id?, session_id?, timestamp?, payload? }` |
| 客户端→服务端 | `message.send` / `media.send` / `ping` |
| 服务端→客户端 | `message.create` / `message.update` / `media.create` / `typing.start` / `typing.stop` / `error` / `pong` |
| 流式策略 | **无 token 级别 streaming**；服务端每条消息完成后下发一条 `message.create`；中间过程用 `payload.thought:true` |
| 消息合并 | 同一 `payload.message_id` 的 `message.create` + `message.update` 合并渲染 |

### 3.2 REST（管理面）

| 用途 | 端点 |
|---|---|
| 获取 WS token | `GET /api/pico/token` |
| 会话 列表 / 详情 / 删除 | `GET /api/sessions` / `GET /api/sessions/:id` / `DELETE /api/sessions/:id` |
| Skills 浏览 / 安装 / 卸载 | `GET /api/skills` / `POST /api/skills/install` / `DELETE /api/skills/:name` |
| Tools 列出 / 启停 | `GET /api/tools` / `PUT /api/tools/:name/state` |

> 这些端点的真实存在性必须在 Phase 1 启动 `picoclaw-launcher` 后用 `curl` 实测。任何缺失的端点由我们在 `backend/pkg/` 内补齐，并在 `backend/PATCHES.md` 记录。

---

## 4. 不再支持的能力（与 v4.2 对照）

| v4.2 能力 | v5.0 处置 |
|---|---|
| Agent 管理（创建/克隆/模型分配） | 删除 |
| 持久化记忆（Long/Short/Working） | 删除 |
| 知识库（FSEvents/Qdrant/Tantivy） | 删除 |
| Vault 快照 / 工作区回滚 | 删除 |
| 任务（Task/Trigger/Run） | 删除 |
| Tool 审批 UI（ADR-036） | 暂停。Pico WS 协议无对应消息类型；如未来要恢复，在 `backend/pkg/channels/pico/` 自行扩协议 |
| Provider / 模型路由 UI | 删除。改由 `~/.picoclaw/config.json` + `.security.yml` 配置 |
| Channels（Telegram / Lark / …） | 由 picoclaw 直接承担，前端不暴露管理面 |
| macOS Keychain / FSEvents / sandbox-exec | 删除 |

---

## 5. 部署形态

### 5.1 开发模式

需要：Go ≥ 1.25、Node ≥ 22 (pnpm)、本地 Ollama（或任意 picoclaw 支持的 provider）。

```bash
# 一次性：编辑 ~/.picoclaw/config.json + .security.yml 配好至少一个 provider
go run ./backend/cmd/picoclaw-launcher onboard       # 首次生成默认 config

# 日常：根目录一条命令并行起前后端
pnpm dev    # 内部 concurrently 跑：
            #   1) go run ./backend/cmd/picoclaw-launcher  → :18790 + :18800
            #   2) pnpm --filter clawx-gui dev             → :5173 (proxy /api/* 与 /pico/ws)
```

打开浏览器到 `http://localhost:5173`。

### 5.2 生产 / 单机模式

```bash
make build    # 产出：build/picoclaw-launcher  +  apps/clawx-gui/dist/
./build/picoclaw-launcher -webroot ./apps/clawx-gui/dist -no-browser
```

launcher 同时托管前端静态资产 + 后端 API。无需 docker，无需 nginx。

---

## 6. 技术栈

| 层 | 技术 | 备注 |
|---|---|---|
| 前端 UI | React 19 + TypeScript 5 | 沿用 |
| 前端构建 | Vite 6 | 沿用 |
| 路由 | react-router-dom 7 | 沿用 |
| Markdown | react-markdown + remark-gfm + highlight.js | 沿用 |
| 前端测试 | vitest 4 + @testing-library/react + jsdom | 沿用 |
| 后端语言 | Go ≥ 1.25 | **新增** |
| 后端测试 | `go test ./...`（picoclaw 自带 Go test 基线） | **新增** |
| 进程管理 | concurrently（dev）；裸二进制（prod） | 不用 systemd / launchd / docker |

**已移除**：Rust toolchain、cargo、所有 `clawx-*` crate、`@tauri-apps/cli`、Tauri Rust 桥接、docker / docker-compose。

---

## 7. 安全模型

picoclaw（Go 进程）独占安全责任，配置在 `backend/`。前端约束：

1. WS / REST 全部走 loopback (`127.0.0.1:18790`) 或受信内网
2. Pico token 只在内存或 `localStorage` 暂存，不进 cookie，不回传任何外部域
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
