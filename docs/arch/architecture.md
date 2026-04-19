# ClawX Architecture v5.0

**日期:** 2026-04-20 | **对应 ADR:** ADR-037 | **取代:** v4.2

> **重大变更**：自 v5.0 起，ClawX 不再持有自研 Rust 后端。后端由开源项目 [sipeed/picoclaw](https://github.com/sipeed/picoclaw) 承担，本仓库只负责"对话前端"。详见 [ADR-037](./decisions.md#adr-037-2026-04-20-全面迁移至-picoclaw-后端删除全部-rust-代码)。

---

## 1. 架构总览

```
┌─────────────────────────────────────────────────────────────┐
│ Browser (any modern browser, no Tauri shell)               │
│   ClawX Web UI (React + Vite + TypeScript)                  │
│   ├── ChatPage         (核心，唯一保留的核心页面)           │
│   ├── ConnectorsPage   (薄壳：picoclaw /api/skills 浏览)    │
│   └── SettingsPage     (Pico token / Gateway URL 配置)      │
└─────────┬───────────────────────────────────┬───────────────┘
          │ HTTPS REST                        │ WebSocket
          │ /api/pico/token, /api/sessions,   │ /pico/ws?session_id=
          │ /api/skills, /api/tools           │ subprotocol: token.<…>
          ▼                                   ▼
┌─────────────────────────────────────────────────────────────┐
│ picoclaw (Go binary，外部进程)                              │
│   ├── Launcher WebUI   :18800   (我们 不 用，可禁用)         │
│   └── Gateway          :18790   (前端唯一对接点)             │
│                                                             │
│   内部能力 (我们不直接控制，仅通过配置使用)：               │
│   • 30+ LLM providers (Anthropic / OpenAI / Ollama …)       │
│   • Tool-use loop                                           │
│   • MCP client (stdio / SSE / HTTP)                         │
│   • Skills (markdown skill files via ClawHub registry)      │
│   • Hooks / SubTurn / Steering                              │
│   • Channels: Pico, Pico-Client, Telegram, Discord, …       │
│   • Cron scheduler                                          │
└─────────────────────────────────────────────────────────────┘
```

**唯一要点**：本仓库的全部代码 = `apps/clawx-gui/`。其它一切都已删除。

---

## 2. 仓库布局（v5.0 之后）

```
frank_claw/
├── apps/
│   └── clawx-gui/              # 唯一的子项目
│       ├── src/
│       │   ├── pages/
│       │   │   ├── ChatPage.tsx
│       │   │   ├── ConnectorsPage.tsx
│       │   │   └── SettingsPage.tsx
│       │   ├── components/     # 仅保留 chat 相关
│       │   ├── lib/
│       │   │   ├── pico-client.ts   # REST 封装
│       │   │   ├── pico-socket.ts   # WS 封装
│       │   │   ├── chat-store.ts    # message-id merge store
│       │   │   ├── types.ts         # PicoMessage 等协议类型
│       │   │   └── store.tsx        # React context
│       │   └── ...
│       ├── package.json        # 已移除 @tauri-apps/cli
│       └── vite.config.ts
├── docs/
│   ├── arch/                   # v5.0 文档（本目录）
│   ├── prd/                    # 待重写（与 v5.0 形态不一致，标记 DEPRECATED）
│   └── superpowers/plans/2026-04-20-picoclaw-migration.md
├── docker-compose.yml          # 一键启动 picoclaw + 前端
├── README.md
└── AGENTS.md
```

**已移除**：`crates/`、`apps/clawx-service/`、`apps/clawx-cli/`、`apps/clawx-gui/src-tauri/`、`Cargo.{toml,lock}`、`rust-toolchain.toml`、`clippy.toml`、`rustfmt.toml`、`target/`。

---

## 3. 协议层

详见 [api-design.md](./api-design.md)。摘要：

### 3.1 WebSocket（聊天主通道）

| 项 | 值 |
|---|---|
| URL | `ws[s]://<gateway-host>:18790/pico/ws?session_id=<uuid>` |
| 鉴权 | Sec-WebSocket-Protocol: `token.<value>` |
| Token 来源 | `GET /api/pico/token` 返回 `{token, ws_url, enabled}` |
| 消息封套 | `{type, id?, session_id?, timestamp?, payload?}` |
| 客户端→服务端 | `message.send` / `media.send` / `ping` |
| 服务端→客户端 | `message.create` / `message.update` / `media.create` / `typing.start` / `typing.stop` / `error` / `pong` |
| 流式策略 | **无 token 级别 streaming**；服务端每条消息完成后下发一条 `message.create`；中间过程用 `payload.thought:true` 表达 |
| 消息合并 | 同一 `payload.message_id` 的 `message.create` + `message.update` 合并渲染 |

### 3.2 REST（管理面）

仅消费 picoclaw 上游已有端点，**不自建任何后端**：

| 用途 | 端点 |
|---|---|
| 获取 WS token | `GET /api/pico/token` |
| 会话列表 / 详情 / 删除 | `GET /api/sessions[?offset&limit]` / `GET /api/sessions/:id` / `DELETE /api/sessions/:id` |
| Skills 浏览 / 安装 | `GET /api/skills` / `GET /api/skills/:name` / `POST /api/skills/install` / `DELETE /api/skills/:name` |
| Tools 开关 | `GET /api/tools` / `PUT /api/tools/:name/state` |

会话**创建** = 直接用前端生成的新 `session_id` 打开 WS；首条 `message.send` 由服务端隐式落库。

---

## 4. 不再支持的能力（与 v4.2 对照）

| v4.2 能力 | v5.0 处置 |
|---|---|
| Agent 管理（创建/克隆/模型分配） | 删除。picoclaw 把"角色"视作 Skill / 配置，不暴露管理面 |
| 持久化记忆（Long/Short/Working） | 删除。picoclaw 自有内存模型，不开放给客户端 |
| 知识库（FSEvents/Qdrant/Tantivy） | 删除 |
| Vault 快照 / 工作区回滚 | 删除 |
| 任务（Task/Trigger/Run） | 删除。picoclaw 内部 cron 不暴露管理面 |
| Tool 审批 UI（Phase-1 ADR-036） | 删除。Pico WS 协议无 `tool_use`/`approval` 消息类型 |
| Provider / 模型路由 UI | 删除。改由 `~/.picoclaw/config.json` + `.security.yml` 配置 |
| Channels（Telegram/Lark/…） | 由 picoclaw 直接承担，前端不暴露管理面 |
| macOS Keychain / FSEvents / sandbox-exec | 删除。picoclaw 自有跨平台安全模型 |

---

## 5. 部署形态

### 5.1 开发模式

```bash
# 终端 1：启 picoclaw
docker compose up picoclaw

# 终端 2：起前端 dev server
cd apps/clawx-gui && pnpm dev   # http://localhost:5173
```

前端通过 Vite 代理把 `/api/*` 与 `/pico/ws` 转发到 `http://127.0.0.1:18790`。

### 5.2 生产 / 单机模式

两种部署等价：
1. **picoclaw 托管前端**：`pnpm build` 输出 `dist/`，由 picoclaw launcher 静态托管。
2. **独立 nginx**：前端构建产物部署到 nginx；nginx 反代 picoclaw gateway。

不再有 macOS launchd / Tauri .app 打包。

---

## 6. 技术栈

| 层 | 技术 | 备注 |
|---|---|---|
| UI | React 19 + TypeScript 5 | 沿用 |
| 构建 | Vite 6 | 沿用 |
| 路由 | react-router-dom 7 | 沿用 |
| Markdown | react-markdown + remark-gfm + highlight.js | 沿用 |
| 测试 | vitest + @testing-library/react + jsdom | 沿用 |
| 后端运行时 | picoclaw (Go ≥ 1.25) | **新增外部依赖** |
| 容器 | docker / docker-compose | 用于固定 picoclaw 版本 |

**已移除**：Rust toolchain、cargo、所有 `clawx-*` crate、`@tauri-apps/cli`、Tauri Rust 桥接。

---

## 7. 安全模型

picoclaw 进程独占安全责任。前端约束：

1. WS / REST 全部走 loopback (`127.0.0.1:18790`) 或受信内网
2. Pico token 只在 `localStorage` 暂存，不进 cookie，不回传任何外部域
3. 前端不直接持有 LLM API Key —— 全部托管在 picoclaw `.security.yml`
4. 前端不实现任何沙箱 / 命令执行，所有 tool 调用在 picoclaw 服务端完成

---

## 8. 历史文档

以下 v4.2 文档保留为历史参考，**与 v5.0 不再一致**：

- [autonomy-architecture.md](./autonomy-architecture.md) — DEPRECATED
- [memory-architecture.md](./memory-architecture.md) — DEPRECATED
- [security-architecture.md](./security-architecture.md) — DEPRECATED
- [data-model.md](./data-model.md) — DEPRECATED
- [crate-dependency-graph.md](./crate-dependency-graph.md) — DEPRECATED
