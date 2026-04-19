# ClawX API 设计 v5.0 (picoclaw consumer profile)

**日期:** 2026-04-20 | **对应架构:** v5.0 | **取代:** v4.2

> 自 v5.0 起，本仓库**不再实现**任何后端 API。本文档只描述前端**消费** picoclaw 上游接口的契约与约束。picoclaw 协议的权威来源是其源码（见 §6 引用）；本文档跟随上游的实际行为，不做规范扩展。

---

## 1. 总览

| 项 | 值 |
|---|---|
| 后端进程 | picoclaw（外部 Go 二进制） |
| Gateway 监听 | `127.0.0.1:18790`（默认；环境变量 `PICOCLAW_GATEWAY_HOST`/`_PORT` 可覆盖） |
| Launcher WebUI | `:18800`（**前端不消费**，部署时建议 `-public=false` 或不启动） |
| 鉴权根 | Pico Token（每个安装独立；通过 launcher 设置） |
| 传输 | HTTP/JSON (REST) + WebSocket (chat) |
| Token 获取 | `GET /api/pico/token` → `{ token, ws_url, enabled }` |

---

## 2. 鉴权

1. 前端启动时调用 `GET /api/pico/token`：
   - 若 `enabled === false`：跳到 `SettingsPage` 提示用户在 picoclaw launcher 启用 Pico 通道。
   - 若 `enabled === true`：把 `token` 写入内存 store（**不要**写 cookie；可选写 `localStorage` 仅作 dev 重启缓存）。
2. WebSocket 连接：`new WebSocket(wsUrl + "?session_id=<uuid>", ["token." + token])`
3. REST 调用：所有 `/api/*` 请求带 `Authorization: Bearer <token>` 头。

> **不要**通过 query 传 token（`?token=…`），picoclaw 仅在 `AllowTokenQuery=true` 时接受，默认关闭，且 query 易出现在日志里。

---

## 3. WebSocket 协议（聊天主通道）

### 3.1 端点

```
ws[s]://<gateway>:18790/pico/ws?session_id=<uuid>
Sec-WebSocket-Protocol: token.<token>
```

`session_id` 由前端 `crypto.randomUUID()` 生成；多个标签页共用 `session_id` 会被 picoclaw 视作同会话广播。

### 3.2 消息封套

```ts
interface PicoMessage {
  type: PicoMessageType;
  id?: string;          // client-generated request id; server echoes in `error.payload.request_id`
  session_id?: string;
  timestamp?: number;   // unix millis
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
- `payload.thought === true` 渲染为"思考过程"次级气泡（次要样式、可折叠）；`thought === false` 或缺省视为最终回复。
- 没有 `done` 事件。`typing.stop` 是 turn 结束的辅助信号；新一条非 `thought` 的 `message.create` 也意味着上一条已完成。
- 媒体消息（`media`）本期不实现，收到 `media.create` 时仅记录、不渲染。

### 3.5 错误处理

- 收到 `error` 时，若 `payload.request_id` 与某条乐观渲染的 `message.send` 的 `id` 匹配，回滚那条用户消息并提示。
- WebSocket 关闭码 1000 视为正常；其它码触发指数退避重连（500ms → 30s 上限）。重连后用同一 `session_id`，picoclaw 会沿用历史。

### 3.6 心跳

每 25s 发送 `{ type: "ping", id: <nonce> }`。若 60s 内未收到 `pong`，主动断开重连。

---

## 4. REST 端点（管理面）

仅消费如下集合，按 `web/frontend/src/api/*.ts` 上游契约调用：

### 4.1 Token

| Method | Path | Body | Resp |
|---|---|---|---|
| GET | `/api/pico/token` | — | `{ token: string, ws_url: string, enabled: boolean }` |
| POST | `/api/pico/token` | `{ regenerate: true }` | 同上 |

### 4.2 Sessions

| Method | Path | Resp |
|---|---|---|
| GET | `/api/sessions?offset=0&limit=50` | `SessionSummary[]` |
| GET | `/api/sessions/:id` | `SessionDetail`（含历史 messages） |
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
| GET | `/api/tools` | 列出可用工具 + 启用状态 |
| PUT | `/api/tools/:name/state` | 启停（body: `{ enabled: boolean }`） |

---

## 5. 不再提供的端点

以下 v4.2 端点**已永久移除**（详见 [decisions.md ADR-037](./decisions.md#adr-037-2026-04-20-全面迁移至-picoclaw-后端删除全部-rust-代码)）：

- `/agents` 全族
- `/conversations`、`/conversations/:id/messages` 全族（被 picoclaw `/api/sessions` + `/pico/ws` 取代）
- `/memories` 全族
- `/knowledge` 全族
- `/vault` 全族
- `/tasks`、`/task-runs`、`/task-triggers` 全族
- `/models`、`/usage` 全族
- `/system/*` 全族
- `/channels` 全族
- `/tools/approval/:id`（Pico WS 协议无 approval 概念）

---

## 6. 上游协议引用

权威来源（写客户端时必须对照源码，不能仅按本文档实现）：

| 文件 | 内容 |
|---|---|
| `pkg/channels/pico/protocol.go` | `PicoMessage`、所有 `type` 常量、`PayloadKey*` |
| `pkg/channels/pico/pico.go` | `handleWebSocket`、`authenticate`、广播策略 |
| `pkg/config/defaults.go` | gateway 端口默认值 |
| `web/frontend/src/api/pico.ts` | token 接口契约 |
| `web/frontend/src/api/sessions.ts` | sessions REST 契约 |
| `web/frontend/src/features/chat/protocol.ts` | 浏览器侧消息分派参考实现 |

---

## 7. 版本兼容策略

picoclaw 自述 "v1.0 之前不要上生产"，协议字段可能微调。本仓库做法：

1. 在 `package.json` 中通过 `docker-compose.yml` 固定 picoclaw 镜像 tag（精确到 patch）。
2. 升级 picoclaw 前，跑一次 `apps/clawx-gui` 的 vitest 协议合约测试（mock WS server 模拟全部 7 种 server-to-client 消息类型）。
3. 协议出现破坏性变更时，本仓库版本号同步抬升 minor。
