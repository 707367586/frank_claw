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
| `backend/hermes_bridge/bridge/hermes_runner.py` | 协议中立的 turn 事件适配器（`typing.start` → `message.create` → `typing.stop`），不导入 hermes 内部 |
| `backend/hermes_bridge/bridge/hermes_factory.py` | **唯一**导入 hermes-agent 内部符号（`run_agent.AIAgent` 等）的文件；升级时先改这里 |

---

## 7. 演进策略

- 协议字段任何变化都同步 `apps/clawx-gui/src/lib/hermes-types.ts` + 对应 vitest 协议合约测试 + `backend/tests/test_ws_protocol.py`。
- hermes-agent 升级：改 `pyproject.toml` 的 git SHA，`uv lock`，跑 `uv run pytest` + `pnpm test`。冲突只改 `hermes_factory.py`。
- 协议破坏性变更：本仓库 minor 抬升，同步记入 `docs/arch/decisions.md`。
