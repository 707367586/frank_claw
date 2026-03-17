# ClawX API 设计

**版本:** 3.0
**日期:** 2026年3月17日

---

## 1. API 概览

ClawX API 层基于 **Axum** 框架，提供 RESTful HTTP 接口。API 同时服务于：
- SwiftUI GUI（通过 FFI 或本地 HTTP）
- CLI 工具
- 移动端远程访问

### 1.1 设计原则

| 原则 | 说明 |
|------|------|
| RESTful | 资源导向，标准 HTTP 方法 |
| JSON | 请求/响应统一使用 JSON |
| 流式支持 | LLM 响应支持 SSE (Server-Sent Events) |
| 本地监听 | 默认绑定 `127.0.0.1`，远程需通过安全隧道 |

### 1.2 基础地址

```
http://127.0.0.1:{port}/api/v1
```

端口在 `~/.clawx/config.toml` 中配置，默认 `19200`。

---

## 2. 核心 API 端点

### 2.1 Agent 管理

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/agents` | 列出所有 Agent |
| `POST` | `/agents` | 创建 Agent |
| `GET` | `/agents/:id` | 获取 Agent 详情 |
| `PUT` | `/agents/:id` | 更新 Agent 配置 |
| `DELETE` | `/agents/:id` | 删除 Agent |
| `POST` | `/agents/:id/clone` | 克隆 Agent |

#### 创建 Agent 请求示例

```json
POST /api/v1/agents
{
  "name": "编程助手",
  "role": "developer",
  "system_prompt": "你是一个专业的编程助手...",
  "model_id": "provider-001",
  "capabilities": ["write_code", "run_shell"]
}
```

#### Agent 响应格式

```json
{
  "id": "agent-uuid-001",
  "name": "编程助手",
  "role": "developer",
  "status": "idle",
  "model_id": "provider-001",
  "capabilities": ["write_code", "run_shell"],
  "last_active_at": "2026-03-17T10:30:00Z",
  "created_at": "2026-03-01T00:00:00Z"
}
```

### 2.2 对话管理

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/agents/:id/conversations` | 列出 Agent 的对话 |
| `POST` | `/agents/:id/conversations` | 创建新对话 |
| `GET` | `/conversations/:id` | 获取对话详情与消息 |
| `DELETE` | `/conversations/:id` | 删除对话 |
| `POST` | `/conversations/:id/messages` | 发送消息 (返回 SSE 流) |
| `POST` | `/conversations/:id/messages/:msg_id/regenerate` | 重新生成回复 |

#### 发送消息（流式响应）

```
POST /api/v1/conversations/{conv_id}/messages
Content-Type: application/json

{
  "content": "帮我写一个冒泡排序",
  "attachments": []
}

Response: text/event-stream
data: {"type": "delta", "content": "好的"}
data: {"type": "delta", "content": "，我来"}
data: {"type": "delta", "content": "帮你写..."}
data: {"type": "tool_call", "name": "write_code", "args": {...}}
data: {"type": "done", "message_id": "msg-uuid-001", "usage": {"input_tokens": 150, "output_tokens": 320}}
```

### 2.3 记忆管理

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/memories` | 查询记忆（支持 scope/type/keyword 过滤） |
| `GET` | `/memories/:id` | 获取单条记忆 |
| `PUT` | `/memories/:id` | 编辑记忆 |
| `DELETE` | `/memories/:id` | 删除记忆 |
| `POST` | `/memories/:id/pin` | 永久保留 |
| `POST` | `/memories/:id/unpin` | 取消永久保留 |

### 2.4 知识库

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/knowledge/sources` | 列出知识源 |
| `POST` | `/knowledge/sources` | 添加知识源文件夹 |
| `DELETE` | `/knowledge/sources/:id` | 移除知识源 |
| `POST` | `/knowledge/search` | 混合检索 |
| `GET` | `/knowledge/stats` | 索引统计 |

#### 混合检索请求

```json
POST /api/v1/knowledge/search
{
  "query": "去年 Q3 性能优化方案的结论",
  "agent_id": "agent-001",
  "top_k": 5,
  "source_ids": []
}
```

#### 检索响应

```json
{
  "results": [
    {
      "chunk_id": "chunk-001",
      "document_id": "doc-001",
      "file_path": "/docs/performance-q3.md",
      "content": "Q3 性能优化结论：通过索引重建...",
      "score": 0.92,
      "score_breakdown": {
        "vector": 0.88,
        "bm25": 0.95,
        "rrf": 0.92
      }
    }
  ],
  "search_time_ms": 120
}
```

### 2.5 定时任务

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/tasks` | 列出所有任务 |
| `POST` | `/tasks` | 创建任务 |
| `GET` | `/tasks/:id` | 获取任务详情 |
| `PUT` | `/tasks/:id` | 更新任务 |
| `DELETE` | `/tasks/:id` | 删除任务 |
| `POST` | `/tasks/:id/pause` | 暂停任务 |
| `POST` | `/tasks/:id/resume` | 恢复任务 |

### 2.6 工作区版本管理 (Vault)

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/vault/snapshots` | 列出版本点 |
| `GET` | `/vault/snapshots/:id` | 版本点详情（含变更集） |
| `GET` | `/vault/snapshots/:id/diff` | 差异预览 |
| `POST` | `/vault/snapshots/:id/rollback` | 执行回滚 |
| `POST` | `/vault/snapshots/:id/rollback/file` | 文件级还原 |

### 2.7 模型管理

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/models` | 列出已配置的模型 |
| `POST` | `/models` | 添加模型配置 |
| `PUT` | `/models/:id` | 更新模型配置 |
| `DELETE` | `/models/:id` | 删除模型配置 |
| `POST` | `/models/:id/test` | 测试连通性 |
| `GET` | `/usage` | 用量统计 |

### 2.8 渠道管理

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/channels` | 列出渠道 |
| `POST` | `/channels` | 添加渠道 |
| `PUT` | `/channels/:id` | 更新渠道配置 |
| `DELETE` | `/channels/:id` | 删除渠道 |
| `POST` | `/channels/:id/connect` | 手动连接 |
| `POST` | `/channels/:id/disconnect` | 手动断开 |

### 2.9 系统管理

| Method | Path | 描述 |
|--------|------|------|
| `GET` | `/system/health` | 健康检查 |
| `GET` | `/system/stats` | 系统资源统计 (CPU/内存/磁盘) |
| `GET` | `/system/audit` | 审计日志查询 |
| `POST` | `/system/update/check` | 检查更新 |

---

## 3. 统一错误格式

```json
{
  "error": {
    "code": "AGENT_NOT_FOUND",
    "message": "Agent with id 'agent-001' not found",
    "details": null
  }
}
```

### 错误码规范

| HTTP Status | 错误场景 |
|------------|---------|
| 400 | 请求参数错误 |
| 404 | 资源不存在 |
| 403 | 权限不足（安全策略拦截） |
| 409 | 资源冲突（Agent 正在运行等） |
| 429 | LLM API 限流 |
| 500 | 内部错误 |

---

## 4. FFI 接口设计 (SwiftUI ↔ Rust)

### 4.1 设计原则

- FFI 边界保持 **薄层**，仅传递简单类型
- 复杂对象序列化为 JSON 字符串跨越 FFI 边界
- 异步操作通过回调或 async/await 桥接

### 4.2 核心 FFI 函数

```rust
// Agent 管理
fn create_agent(config_json: &str) -> String;       // -> Result JSON
fn list_agents() -> String;                          // -> Agent[] JSON
fn delete_agent(agent_id: &str) -> String;           // -> Result JSON

// 对话
fn send_message(
    conversation_id: &str,
    content: &str,
    callback: fn(chunk: &str),                       // 流式回调
) -> String;

// 记忆
fn query_memories(filter_json: &str) -> String;      // -> Memory[] JSON

// 知识库
fn search_knowledge(query_json: &str) -> String;     // -> SearchResult[] JSON

// 系统
fn get_system_health() -> String;                    // -> HealthStatus JSON
```

### 4.3 SwiftUI ViewModel 调用模式

```
SwiftUI View
    │ @StateObject
    ▼
ViewModel
    │ async/await
    ▼
FFI Bridge (swift-bridge)
    │ JSON serialize/deserialize
    ▼
Rust Core (clawx-ffi → clawx-runtime)
```

---

## 5. Cloud Relay API（移动端通信）

Cloud Relay 是独立部署的后端服务，负责 iOS 移动端与 Mac 主机之间的消息转发。

### 5.1 基础地址

```
https://relay.clawx.com/api/v1
```

### 5.2 设备管理

| Method | Path | 描述 | 调用方 |
|--------|------|------|--------|
| `POST` | `/devices/register` | 注册设备 | Mac / iOS |
| `DELETE` | `/devices/:id` | 注销设备 | Mac / iOS |
| `GET` | `/devices` | 列出账号下所有设备 | iOS |
| `GET` | `/devices/:id/status` | 查询设备在线状态 | iOS |

#### 设备注册请求

```json
POST /api/v1/devices/register
Authorization: Bearer <account_token>
{
  "device_type": "mac",
  "device_name": "Frank's MacBook Pro",
  "public_key": "<X25519 公钥, Base64>"
}
```

### 5.3 消息转发

Mac 主机通过 **WebSocket (WSS)** 长连接保持在线：

```
WSS wss://relay.clawx.com/ws?device_id={mac_device_id}&token={auth_token}
```

iOS 通过 HTTPS 发送指令，Relay 通过 WSS 转发给 Mac：

| Method | Path | 描述 | 调用方 |
|--------|------|------|--------|
| `POST` | `/relay/send` | 发送加密消息到目标设备 | iOS |
| `GET` | `/relay/pending` | 拉取离线期间的待投递消息 | iOS / Mac |

#### 消息转发请求

```json
POST /api/v1/relay/send
Authorization: Bearer <account_token>
{
  "target_device_id": "mac-device-001",
  "encrypted_payload": "<E2E 加密密文, Base64>",
  "message_type": "agent_request",
  "ttl": 604800
}
```

**说明**：`encrypted_payload` 由发送端使用 X25519 协商的会话密钥加密，Relay 服务无法解密。

### 5.4 推送通知

Mac 产生主动通知时，通过 Relay 转发到 iOS 的 APNs：

| Method | Path | 描述 | 调用方 |
|--------|------|------|--------|
| `POST` | `/relay/notify` | 发送推送通知 | Mac (via WSS) |

```json
{
  "target_device_id": "ios-device-001",
  "notification": {
    "title": "研究助手",
    "body": "arXiv 今日 AI Safety 新论文摘要已生成"
  },
  "encrypted_payload": "<E2E 加密的完整通知内容>"
}
```

### 5.5 密钥交换

首次配对时，Mac 和 iOS 通过 Relay 交换 X25519 公钥：

```
iOS                    Relay                   Mac
 │                       │                       │
 │── POST /key-exchange ─▶│── WSS forward ───────▶│
 │   {ios_public_key}     │                       │
 │                        │◀── WSS response ──────│
 │◀── 200 ────────────────│   {mac_public_key}    │
 │                        │                       │
 │  双方各自用对方公钥 + 自己私钥派生共享密钥       │
 │  后续所有 payload 使用该共享密钥 AES-256-GCM 加密│
```
