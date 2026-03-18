# ClawX API 设计 v4.2

**日期:** 2026-03-18 | **对应架构:** v4.2

---

## 1. 概览

ClawX 的接口设计围绕**本地控制平面**展开：

- 默认传输：Unix Domain Socket `~/.clawx/run/clawx.sock`
- 默认协议：Axum HTTP/JSON，流式输出使用 SSE
- 可选开发模式：`127.0.0.1:19200`
- GUI、CLI 都通过 `clawx-controlplane-client` 访问 `clawx-service`

说明：

- `clawx-api` 是 service 内 server
- `clawx-controlplane-client` 是本地客户端共享访问库
- `clawx-ffi` 只桥接到 `clawx-controlplane-client`，不直接持有 runtime

### 1.1 本地认证

- 默认要求 `Authorization: Bearer <control_token>`
- `control_token` 由 service 启动时生成，存放于 `~/.clawx/run/control.token`
- Socket 与 token 文件均使用 owner-only 权限
- 本地控制平面不是安全豁免区；高风险操作仍回到 Security 审批链

---

## 2. 本地 API 端点

### 2.1 Agent 管理

| Method | Path | 描述 |
|--------|------|------|
| GET/POST | `/agents` | 列出 / 创建 Agent |
| GET/PUT/DELETE | `/agents/:id` | 获取 / 更新 / 删除 Agent |
| POST | `/agents/:id/clone` | 克隆 Agent |

### 2.2 对话

| Method | Path | 描述 |
|--------|------|------|
| GET/POST | `/agents/:id/conversations` | 列出 / 创建对话 |
| GET/DELETE | `/conversations/:id` | 获取 / 删除对话 |
| POST | `/conversations/:id/messages` | 发送消息（SSE 流式响应） |

SSE 事件格式：

- `delta`
- `execution_step`
- `confirmation_required`
- `done`
- `error`

### 2.3 记忆

| Method | Path | 描述 |
|--------|------|------|
| GET | `/memories` | 查询（scope / kind / keyword） |
| POST | `/memories/search` | 语义搜索记忆（v0.1 FTS5，v0.2 向量检索） |
| GET/PUT/DELETE | `/memories/:id` | 获取 / 编辑 / 删除 |
| POST | `/memories/:id/pin` | 固定或取消固定 |

### 2.4 知识库

| Method | Path | 描述 |
|--------|------|------|
| GET/POST/DELETE | `/knowledge/sources` | 知识源管理 |
| POST | `/knowledge/search` | 文档混合检索 |

### 2.5 Vault 与系统

| 模块 | 端点 |
|------|------|
| Vault | `/vault/snapshots`、`/vault/diff`、`/vault/rollback` |
| 模型 | `/models`、`/models/test`、`/usage` |
| 系统 | `/system/health`、`/system/stats`、`/system/audit` |

### 2.6 v0.2+ 扩展端点

以下端点属于扩展执行层，不是 v0.1 闭环前置条件：

| 模块 | 端点 |
|------|------|
| 任务 | `GET/POST /tasks`、`GET/PUT/DELETE /tasks/:id` |
| Trigger | `POST /tasks/:id/triggers`、`PUT/DELETE /task-triggers/:id` |
| Run | `GET /tasks/:id/runs`、`GET /task-runs/:id` |
| 反馈 | `POST /task-runs/:id/feedback` |
| 生命周期 | `POST /tasks/:id/pause`、`/resume`、`/archive` |
| 权限档案 | `GET /agents/:id/permission-profile` |
| 渠道 | `/channels` |
| Skills | `/skills` |

---

## 3. 错误格式

```json
{ "error": { "code": "AGENT_NOT_FOUND", "message": "...", "details": null } }
```

语义约定：

- `400` 参数错误
- `403` 权限不足
- `404` 资源不存在
- `409` 状态冲突
- `423` 资源被占用（如 Run lease 未释放）
- `429` 限流
- `500` 内部错误

---

## 4. FFI 与 CLI 边界

### 4.1 调用链

```text
SwiftUI View / CLI Command
  -> clawx-ffi or CLI front-end
  -> clawx-controlplane-client
  -> clawx-api
  -> clawx-runtime
```

### 4.2 设计约束

1. `clawx-ffi` 与 `clawx-cli` 不持有长期 runtime state
2. 所有状态变更请求都必须经由 `clawx-api`
3. `/tasks` 是主动任务的唯一控制面端点，不再额外暴露 `/schedules`
4. 预览、诊断或测试优先走 mock server / test server，而不是直连 runtime

### 4.3 FFI 示例

```rust
fn create_agent(config_json: &str) -> String;
fn list_agents() -> String;
fn send_message(conversation_id: &str, content: &str, callback: fn(&str)) -> String;
fn query_memories(filter_json: &str) -> String;
fn search_knowledge(query_json: &str) -> String;
fn get_system_health() -> String;
```

---

## 5. 未来平台边界（v0.3+）

Cloud Relay 不是本地控制平面的一部分：

- 认证体系不同
- 传输层不同
- 配额和审计策略也不同

因此它不复用本地 UDS API 命名空间；详细接口在 v0.5 平台设计阶段单独定义。
