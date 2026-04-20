# ClawX API 设计 v5.0 (vendored picoclaw, owned contract)

**日期:** 2026-04-20 | **对应架构:** v5.0 | **取代:** v4.2

> 自 v5.0 起，本仓库不实现独立的 Rust 后端，但 picoclaw 源码已 vendor 到 `backend/`，**所以这套 API 是我们拥有的契约**。Phase 1 完成后将以 `backend/pkg/...` 内的 Go 代码为权威；本文档与代码不一致时，先以代码为准、再修 doc。

---

## 1. 总览

| 项 | 值 |
|---|---|
| 后端进程 | `picoclaw-launcher`（从本仓库 `backend/cmd/picoclaw-launcher` 编译） |
| Gateway 监听 | `127.0.0.1:18790`（`gateway.host` / `gateway.port` in `~/.picoclaw/config.json`） |
| Web Console（上游自带） | `:18800`（默认开。我们**不**用其 UI；可以用 `-webroot` 让 launcher 托管 `apps/clawx-gui/dist/`） |
| 鉴权根 | Pico Token（首次启动由 launcher 生成，存 `~/.picoclaw/config.json`） |
| 传输 | HTTP/JSON (REST) + WebSocket (chat) |
| Token 获取 | `GET /api/pico/token` → `{ token, ws_url, enabled }` |

> Phase 1 必须用 `curl` 实测下面所有路径在我们 vendor 的 picoclaw 二进制里真的存在；任何缺口在 `backend/` 内补 Go 实现。

---

## 2. 鉴权

1. 前端启动时 `GET /api/pico/token`：
   - `enabled === false` → 跳到 SettingsPage 提示用户在 `~/.picoclaw/config.json` 把 `channels.pico.enabled = true` 并重启 launcher。
   - `enabled === true` → 把 `token` 放内存 store。
2. WebSocket：`new WebSocket(wsUrl + "?session_id=<uuid>", ["token." + token])`
3. REST：所有 `/api/*` 请求带 `Authorization: Bearer <token>` 头。

> **不要**用 query 传 token（`?token=…`）：picoclaw 仅在 `AllowTokenQuery=true` 时接受，默认关闭，且 query 易出现在日志里。

---

## 3. WebSocket 协议（聊天主通道）

### 3.1 端点

```
ws[s]://<gateway>:18790/pico/ws?session_id=<uuid>
Sec-WebSocket-Protocol: token.<token>
```

`session_id` 由前端 `crypto.randomUUID()` 生成。多个 tab 共用 `session_id` 会被 picoclaw 视作同会话广播。

### 3.2 消息封套

```ts
interface PicoMessage {
  type: PicoMessageType;
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
| S→C | `media.create` | （媒体回包；本期 chat-only 可忽略） |
| S→C | `typing.start` | `{}` |
| S→C | `typing.stop` | `{}` |
| S→C | `error` | `{ code: string, message: string, request_id?: string }` |
| S→C | `pong` | `{}`（echoes client `id`） |

### 3.4 渲染规则

- **不存在 token 级别 streaming**。服务端每条消息完成后下发一次 `message.create`。
- 同一 `message_id` 后续可有零或多个 `message.update`，前端按 id 合并替换 `content`。
- `payload.thought === true` 渲染为"思考过程"次级气泡；`thought === false` 或缺省视为最终回复。
- 没有 `done` 事件。`typing.stop` 是 turn 结束的辅助信号；新一条非 `thought` 的 `message.create` 也意味着上一条已完成。
- 媒体消息（`media.create`）本期不实现。

### 3.5 错误处理

- 收到 `error` 时，若 `payload.request_id` 与某条乐观渲染的 `message.send` 的 `id` 匹配，回滚那条用户消息并提示。
- WebSocket 关闭码 `1000` 视为正常；其它码触发指数退避重连（500ms → 30s 上限）。重连用同一 `session_id`，picoclaw 沿用历史。

### 3.6 心跳

每 25s 发送 `{ type: "ping", id: <nonce> }`。若 60s 内未收到 `pong`，主动断开重连。

---

## 4. REST 端点（管理面）

> Phase 1 任务 1.4 必须 `curl` 实测每个端点存在 + 返回符合预期的 JSON。任一缺失，加一个 Phase 2 子任务在 `backend/pkg/api/` 补 Go handler + Go 单测，并在 `backend/PATCHES.md` 记录。

### 4.1 Token

| Method | Path | Body | Resp |
|---|---|---|---|
| GET | `/api/pico/token` | — | `{ token: string, ws_url: string, enabled: boolean }` |
| POST | `/api/pico/token` | `{ regenerate: true }` | 同上 |

### 4.2 Sessions

| Method | Path | Resp |
|---|---|---|
| GET | `/api/sessions?offset=0&limit=50` | `SessionSummary[]` |
| GET | `/api/sessions/:id` | `SessionDetail`（含 messages） |
| DELETE | `/api/sessions/:id` | 204 |

```ts
interface SessionSummary {
  id: string;
  title: string;
  preview: string;
  message_count: number;
  created: number;
  updated: number;
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

### 4.3 Skills

| Method | Path | 用途 |
|---|---|---|
| GET | `/api/skills` | 列出已安装 |
| GET | `/api/skills/:name` | 详情 |
| GET | `/api/skills/search?q=` | ClawHub 搜索 |
| POST | `/api/skills/install` | 安装（body: `{ name }`） |
| DELETE | `/api/skills/:name` | 卸载 |

### 4.4 Tools

| Method | Path | 用途 |
|---|---|---|
| GET | `/api/tools` | 列出 + 启用状态 |
| PUT | `/api/tools/:name/state` | 启停（body: `{ enabled: boolean }`） |

---

## 5. 不再提供的端点

以下 v4.2 端点**已永久移除**（详见 [decisions.md ADR-037](./decisions.md#adr-037-2026-04-20-删除-rust-后端将-picoclaw-源码-vendor-进本仓库作为新后端)）：

- `/agents` 全族
- `/conversations`、`/conversations/:id/messages` 全族（被 `/api/sessions` + `/pico/ws` 取代）
- `/memories` 全族
- `/knowledge` 全族
- `/vault` 全族
- `/tasks`、`/task-runs`、`/task-triggers` 全族
- `/models`、`/usage` 全族
- `/system/*` 全族
- `/channels` 全族
- `/tools/approval/:id`（Pico WS 协议无 approval 概念）

---

## 6. 实现引用（vendor 后的本地路径）

权威来源是我们 vendor 进 `backend/` 的 Go 源码：

| 文件（vendor 后路径） | 内容 |
|---|---|
| `backend/pkg/channels/pico/protocol.go` | `PicoMessage`、所有 `type` 常量、`PayloadKey*` |
| `backend/pkg/channels/pico/pico.go` | `handleWebSocket`、`authenticate`、广播策略 |
| `backend/pkg/config/defaults.go` | gateway 端口默认值 |
| `backend/cmd/picoclaw-launcher/` | launcher 二进制入口（含 `/api/*` web console 路由） |
| `backend/web/frontend/src/api/pico.ts` | 上游 token 接口实现参考（我们仅作参考，前端用 TS 重写） |
| `backend/web/frontend/src/api/sessions.ts` | 上游 sessions REST 实现参考 |
| `backend/web/frontend/src/features/chat/protocol.ts` | 上游浏览器侧消息分派参考 |

---

## 7. 演进策略

- 协议字段任何变化都必须同步改 `apps/clawx-gui/src/lib/pico-types.ts` + 对应的 vitest 协议合约测试。
- 上游 picoclaw 同步时，先跑全套测试（`go test ./backend/...` + `pnpm vitest run`）；变更与本地 `PATCHES.md` 冲突时，**保留本地行为**并人工合并。
- 协议出现破坏性变更时，本仓库版本号同步抬升 minor；同时在 `backend/UPSTREAM.md` 记录基线 SHA 移动。
