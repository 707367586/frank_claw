# ClawX 后续开发计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 完成 v0.2 剩余交付（GUI 全页面 + 真实集成 + 验收），然后推进 v0.3 社交闭环（Agent 身份 + Relay + E2E 加密 + 远程对话）。

**Architecture:** ClawX 采用 Rust Workspace 分层单体架构，后端 684 个测试已全部通过。GUI 层使用 Vite + React + TS（当前仅有基础骨架），通过 HTTP API 连接 clawx-service。v0.3 将新增 identity/protocol/relay-client/social 四个 crate 支撑社交网络。

**Tech Stack:** Rust (tokio/axum/sqlx), React 19 + TypeScript + Vite, SQLite, Qdrant embedded, Tantivy, Ed25519, X25519, AES-256-GCM

---

## 现状总结

### 已完成

| 版本 | 交付内容 | 测试数 |
|------|---------|--------|
| v0.1 | Agent CRUD, 对话+SSE, 记忆系统(FTS5+衰减), 知识库(BM25+RRF), Vault, 安全基线(7层), HAL, CLI | 329 |
| v0.2 后端 | 安全执行官(12层), Skills WASM沙箱, Executor多步执行, Permission Gate, Task/Trigger/Run, Scheduler, Attention Policy, IM渠道框架, Qdrant向量检索, Embedding/Reranker | +355=684 |

### v0.2 剩余（后端已完成，缺 GUI + 真实集成）

| 项目 | 状态 | 说明 |
|------|------|------|
| Agent Loop ↔ TaskExecutor | ✅ | A1-A5 已对接 |
| 对话创建任务 (NLP→Task) | ✅ | B1-B4 已完成 |
| 渠道消息全链路 | ⚠️ | C1-C3 ✅，C4 Telegram/Lark 真实 API 未接 |
| GUI 知识库管理页 | ✅ | Phase B Task B3 已完成 |
| GUI 定时任务管理页 | ✅ | Phase B Task B4 已完成 |
| GUI Connectors 管理页 | ✅ | Phase B Task B5 已完成 |
| GUI 设置页 | ✅ | Phase B Task B6 已完成 |
| GUI SSE 流式对接 | ✅ | Phase B Task B1 已完成 |
| 性能基准测试 | ❌ | E1 |
| WASM 沙箱逃逸测试 | ❌ | E2 |

### 当前 GUI 状态（2026-04-13 更新）

`apps/clawx-gui/` 已完成微信风格三栏布局重构，包含全部 6 个核心页面：
- 三栏布局（A栏图标导航 56px + B栏列表 280px + C栏内容自适应）
- 对话页（SSE 流式 + Markdown 渲染 + highlight.js 代码高亮 + DOMPurify XSS 防护）
- 通讯录页（Agent CRUD + 记忆/权限标签页）
- 知识库管理页（知识源列表 + 检索工作台）
- 定时任务管理页（Task/Trigger/Run + 反馈）
- Connectors 管理页（渠道 CRUD + 类型特定配置）
- 设置页（模型管理 + 安全 + 系统健康 + 关于）

---

## 开发路线图

```
Phase A: GUI 基础架构重构（三栏布局 + 路由 + 状态管理）
  ↓
Phase B: GUI 核心页面（对话页 SSE + 通讯录 + 知识库 + 任务 + 设置）
  ↓
Phase C: 真实集成 + 验收（Telegram/Lark API + 性能测试）
  ↓
  === v0.2 交付完成 ===
  ↓
Phase D: v0.3 类型扩展 + 数据库迁移（Identity/Protocol/Social 类型）
  ↓
Phase E: Agent 身份系统（Ed25519 密钥对 + AgentAddress + 名片）
  ↓
Phase F: 通信协议层（信封定义 + X25519 加密 + AES-256-GCM）
  ↓
Phase G: Relay 客户端 + 服务端 MVP
  ↓
Phase H: 社交功能（通讯录 + 添加好友 + 远程 1:1 对话）
  ↓
Phase I: GUI 社交页面 + 集成验收
  ↓
  === v0.3 交付完成 ===
```

---

## Phase A: GUI 基础架构重构 ✅

> **目标：** 从当前的简易两栏布局重构为微信风格三栏布局，建立 GUI 开发基础设施。

### Task A1: 工程基础设施

**Files:**
- Modify: `apps/clawx-gui/package.json`
- Create: `apps/clawx-gui/src/lib/api.ts`
- Create: `apps/clawx-gui/src/lib/types.ts`

- [x] **Step 1:** 安装依赖 — `react-router-dom`、`react-markdown`、`remark-gfm`、`highlight.js`（代码高亮）、`lucide-react`（图标）

```bash
cd apps/clawx-gui && pnpm add react-router-dom react-markdown remark-gfm highlight.js lucide-react
```

- [x] **Step 2:** 创建 API 客户端模块 `src/lib/api.ts`
  - 封装 fetch wrapper（自动加 Authorization header、错误处理）
  - 封装 SSE 连接函数（EventSource wrapper + 自动重连）
  - 所有 API 端点函数：agents、conversations、messages、memories、knowledge、tasks、triggers、runs、channels、skills、models、system
  - 基础类型定义从 clawx-types 手动映射到 TypeScript

- [x] **Step 3:** 创建 TypeScript 类型定义 `src/lib/types.ts`
  - Agent / Conversation / Message / Memory / KnowledgeSource / Task / Trigger / Run / Channel / Skill / ModelProvider
  - 对齐 `crates/clawx-types/src/` 中的 Rust 类型

- [x] **Step 4:** 验证 API 客户端可以连接 clawx-service 并获取 Agent 列表

- [x] **Step 5:** Commit

### Task A2: 三栏布局框架

**Files:**
- Rewrite: `apps/clawx-gui/src/App.tsx`
- Create: `apps/clawx-gui/src/layouts/AppLayout.tsx`
- Create: `apps/clawx-gui/src/components/NavBar.tsx` (A栏)
- Create: `apps/clawx-gui/src/components/ListPanel.tsx` (B栏)
- Create: `apps/clawx-gui/src/styles/layout.css`

- [x] **Step 1:** 实现 A 栏图标导航栏（56px 固定宽度）
  - 图标：对话、通讯录、知识库、定时任务、Connectors、设置
  - 选中态高亮
  - 底部设置按钮

- [x] **Step 2:** 实现 B 栏列表面板（280px）
  - 顶部搜索栏
  - 根据 A 栏选中项动态切换内容
  - 列表项选中态

- [x] **Step 3:** 实现 C 栏主内容区（自适应宽度）
  - 根据 B 栏选中项渲染对应内容

- [x] **Step 4:** 配置 React Router
  - `/` → 对话页
  - `/contacts` → 通讯录
  - `/knowledge` → 知识库
  - `/tasks` → 定时任务
  - `/connectors` → Connectors
  - `/settings` → 设置

- [x] **Step 5:** 验证三栏布局在浏览器中正确渲染，A/B/C 栏宽度和交互符合预期

- [x] **Step 6:** Commit

---

## Phase B: GUI 核心页面 ✅

> **目标：** 实现全部 v0.2 需要的 GUI 页面。

### Task B1: 对话页（SSE 流式 + Markdown 渲染）

**Files:**
- Create: `apps/clawx-gui/src/pages/ChatPage.tsx`
- Create: `apps/clawx-gui/src/components/MessageBubble.tsx`
- Create: `apps/clawx-gui/src/components/ConversationList.tsx`
- Create: `apps/clawx-gui/src/components/ChatInput.tsx`

- [x] **Step 1:** B 栏对话列表
  - 按最后消息时间排序
  - 显示 Agent 名称、最后消息摘要、时间
  - 新建对话按钮
  - 搜索过滤

- [x] **Step 2:** C 栏聊天区域
  - 消息列表（用户消息右侧、Agent 消息左侧）
  - Markdown 渲染（react-markdown + remark-gfm）
  - 代码块语法高亮（highlight.js）
  - 自动滚动到底部

- [x] **Step 3:** SSE 流式输出
  - `POST /conversations/:id/messages?stream=true` → EventSource 接收
  - 逐字显示 Agent 回复
  - 支持中断生成（abort controller）
  - 显示 execution_step 事件（多步任务进度）

- [x] **Step 4:** 消息操作
  - 复制消息内容
  - 重新生成回复

- [x] **Step 5:** 启动 dev server + clawx-service，验证完整对话流程（发消息 → SSE 流式回复 → Markdown 渲染）

- [x] **Step 6:** Commit

### Task B2: 通讯录页（Agent 管理）

**Files:**
- Create: `apps/clawx-gui/src/pages/ContactsPage.tsx`
- Create: `apps/clawx-gui/src/components/AgentCard.tsx`
- Create: `apps/clawx-gui/src/components/AgentDetail.tsx`
- Create: `apps/clawx-gui/src/components/AgentForm.tsx`

- [x] **Step 1:** B 栏 Agent 列表
  - 分组显示「我的 Agent」
  - Agent 卡片：头像、名称、角色、状态指示灯（idle/working/error）
  - 搜索过滤
  - 顶部「+ 创建 Agent」按钮

- [x] **Step 2:** C 栏 Agent 详情页
  - 基本信息（名称、角色、System Prompt、绑定模型）
  - 记忆标签页（展示 Agent 记忆列表 + 搜索 + 删除/固定）
  - 权限标签页（展示 Permission Profile 的 5 个能力维度 + 当前 Trust Level）
  - 知识库标签页（已关联知识源列表）

- [x] **Step 3:** Agent CRUD 表单
  - 创建 Agent：名称、角色、System Prompt、模型选择
  - 编辑 Agent：修改配置
  - 删除 Agent：二次确认对话框
  - 克隆 Agent

- [x] **Step 4:** 验证 Agent 创建 → 查看详情 → 编辑 → 开始对话完整流程

- [x] **Step 5:** Commit

### Task B3: 知识库管理页

**Files:**
- Create: `apps/clawx-gui/src/pages/KnowledgePage.tsx`
- Create: `apps/clawx-gui/src/components/SourceList.tsx`
- Create: `apps/clawx-gui/src/components/SearchWorkbench.tsx`

- [x] **Step 1:** B 栏知识源列表
  - 显示已添加知识源（文件夹路径 + 文档数 + 索引状态）
  - 「+ 添加知识源」按钮 → 选择文件夹
  - 删除知识源

- [x] **Step 2:** C 栏检索工作台
  - 搜索输入框
  - 搜索结果列表（Chunk 内容 + 来源文件 + 相关度分数）
  - 点击结果定位到源文件

- [x] **Step 3:** 知识源详情
  - 文档列表（文件名 + chunk 数 + 最后索引时间）
  - 手动重新索引按钮
  - 索引进度指示

- [x] **Step 4:** 验证添加知识源 → 自动索引 → 检索返回结果

- [x] **Step 5:** Commit

### Task B4: 定时任务管理页

**Files:**
- Create: `apps/clawx-gui/src/pages/TasksPage.tsx`
- Create: `apps/clawx-gui/src/components/TaskList.tsx`
- Create: `apps/clawx-gui/src/components/TaskDetail.tsx`
- Create: `apps/clawx-gui/src/components/RunHistory.tsx`

- [x] **Step 1:** B 栏任务列表
  - 显示所有 Task：名称、关联 Agent、状态（active/paused/archived）、下次执行时间
  - 搜索 + 按状态过滤
  - 「+ 创建任务」按钮

- [x] **Step 2:** C 栏任务详情
  - 任务基本信息（名称、目标、关联 Agent）
  - 触发器列表（Trigger 类型 + Cron 表达式 + 下次触发时间）
  - 添加/编辑/删除触发器

- [x] **Step 3:** Run 执行历史
  - Run 列表（状态 + 开始时间 + 耗时 + 步骤数）
  - Run 详情（每步 ExecutionStep：action / tool / evidence / risk_reason）
  - 反馈按钮（accepted / rejected / mute_forever / reduce_frequency）

- [x] **Step 4:** 任务操作
  - 暂停 / 恢复 / 归档
  - 编辑任务配置
  - 删除任务（二次确认）

- [x] **Step 5:** 验证创建任务 → 添加触发器 → 查看 Run 历史 → 提交反馈

- [x] **Step 6:** Commit

### Task B5: Connectors 管理页

**Files:**
- Create: `apps/clawx-gui/src/pages/ConnectorsPage.tsx`
- Create: `apps/clawx-gui/src/components/ChannelCard.tsx`
- Create: `apps/clawx-gui/src/components/ChannelForm.tsx`

- [x] **Step 1:** B 栏渠道列表
  - 显示已配置渠道：类型图标 + 名称 + 状态灯（connected/disconnected/error）+ 绑定的 Agent
  - 「+ 添加渠道」按钮

- [x] **Step 2:** C 栏渠道配置
  - 渠道类型选择（Telegram / Lark / Slack / Discord / WhatsApp / WeChat Enterprise）
  - 配置表单（根据类型动态显示：bot_token / app_id + app_secret 等）
  - 绑定 Agent 选择器
  - 连接/断开按钮
  - 连接状态实时显示

- [x] **Step 3:** 验证添加 Telegram 渠道 → 配置 → 绑定 Agent → 连接

- [x] **Step 4:** Commit

### Task B6: 设置页

**Files:**
- Create: `apps/clawx-gui/src/pages/SettingsPage.tsx`
- Create: `apps/clawx-gui/src/components/settings/ModelSettings.tsx`
- Create: `apps/clawx-gui/src/components/settings/SecuritySettings.tsx`
- Create: `apps/clawx-gui/src/components/settings/SystemHealth.tsx`

- [x] **Step 1:** B 栏设置分类列表
  - 模型管理
  - 安全配置
  - 系统健康
  - 关于

- [x] **Step 2:** 模型管理
  - Provider 列表（名称 + 类型 + 状态）
  - 添加 Provider 表单（选择类型 → 填入 API Key + Base URL）
  - 测试连通性按钮
  - 删除 Provider

- [x] **Step 3:** 安全配置
  - 网络白名单管理（域名列表 + 添加/删除）
  - DLP 开关
  - Prompt 注入防御开关

- [x] **Step 4:** 系统健康
  - `/system/health` 健康状态展示
  - `/system/stats` 统计信息（Agent 数、记忆数、知识库文档数、磁盘用量）
  - launchd 服务状态

- [x] **Step 5:** 验证模型配置 → 测试连通 → 查看系统健康

- [x] **Step 6:** Commit

---

## Phase C: 真实集成 + v0.2 验收 ✅

> **目标：** 完成真实外部集成和性能验收，交付 v0.2。

### Task C1: Telegram Bot 真实 API 接入

**Files:**
- Modify: `crates/clawx-channel/src/lib.rs`（或新建 `telegram.rs`）

- [x] **Step 1:** 实现 Telegram Bot Long Polling
  - `getUpdates` API 轮询
  - 消息解析（text / document / photo）
  - 通过 ChannelAdapter trait 接入

- [x] **Step 2:** 实现出站消息
  - `sendMessage` API
  - Markdown 格式化
  - 文件发送（`sendDocument`）

- [x] **Step 3:** 端到端测试：用真实 Telegram Bot Token，发消息 → Agent 回复 → Telegram 收到

- [x] **Step 4:** Commit

### Task C2: 飞书/Lark WebSocket 接入

**Files:**
- Modify: `crates/clawx-channel/src/lib.rs`（或新建 `lark.rs`）

- [x] **Step 1:** 实现 Lark WebSocket 长连接
  - 获取 tenant_access_token
  - WebSocket 连接 + 心跳
  - 消息事件解析

- [x] **Step 2:** 实现出站消息
  - `/im/v1/messages` API 发送消息
  - Rich text 卡片消息

- [x] **Step 3:** 端到端测试

- [x] **Step 4:** Commit

### Task C3: 性能基准 + 安全验收

- [x] **Step 1:** 编写性能基准测试框架
  - 冷启动时间
  - 记忆召回 P50/P95
  - 知识库检索 P50/P95
  - 多步执行 10 步完成时间
  - 定时任务触发精度

- [x] **Step 2:** 启动 Embedding 服务（TEI + Qwen3-VL-Embedding-2B），运行向量检索性能测试

- [x] **Step 3:** WASM 沙箱逃逸测试（需编写真实 WASM 测试二进制）

- [x] **Step 4:** 生成 v0.2 验收报告

- [x] **Step 5:** Commit

---

## Phase D: v0.3 类型扩展 + 数据库迁移

> **目标：** 为社交网络能力铺设类型基础和数据表。
> **对齐：** `docs/superpowers/specs/2026-03-30-agent-social-network-design.md`

### Task D1: clawx-types 社交类型扩展

**Files:**
- Create: `crates/clawx-types/src/identity.rs`
- Create: `crates/clawx-types/src/protocol.rs`
- Create: `crates/clawx-types/src/social.rs`
- Modify: `crates/clawx-types/src/lib.rs`
- Modify: `crates/clawx-types/src/agent.rs`
- Modify: `crates/clawx-types/src/traits.rs`

- [ ] **Step 1:** 新建 `identity.rs`
  - AgentAddress（`<agent_pubkey_hash>@<instance_id>`）+ 解析/格式化
  - AgentKeypair（Ed25519 公私钥对）
  - InstanceKeypair（实例级密钥对）
  - ProfileCard（名称/头像/简介/标签/可见性/收费策略/在线状态）+ Ed25519 签名
  - Visibility 枚举（public / invite_only / private）
  - OnlineStatus 枚举（online / offline / busy）
  - PricingPolicy 枚举（free / subscription / per_use）

- [ ] **Step 2:** 新建 `protocol.rs`
  - Envelope（from / to / timestamp / envelope_sig / type / encrypted_payload）
  - EnvelopeType 枚举（direct / group / rpc）
  - Payload（message_id / content_type / body / reply_to / metadata）
  - EncryptedPayload（ciphertext + nonce + ephemeral_pubkey）
  - SessionKey（X25519 协商结果）

- [ ] **Step 3:** 新建 `social.rs`
  - Contact（远程 Agent 名片缓存）
  - ContactRequest（添加好友请求）
  - ContactRequestStatus 枚举（pending / accepted / rejected）
  - Group / GroupMember / GroupRole
  - ConversationType 扩展（local / remote / delegate / group）
  - FriendSource 枚举（search / invite / group）

- [ ] **Step 4:** 扩展 `agent.rs` — 增加 public_key / visibility / bio / tags / pricing_policy / online_status
- [ ] **Step 5:** 扩展 `traits.rs` — IdentityPort / ProtocolPort / RelayClientPort / ContactRegistryPort / GroupRegistryPort
- [ ] **Step 6:** 单元测试：AgentAddress 解析、ProfileCard 签名验签、Envelope 序列化、类型往返
- [ ] **Step 7:** Commit

### Task D2: 数据库迁移

**Files:**
- Modify: `crates/clawx-runtime/src/db.rs`

- [ ] **Step 1:** 新增数据表
  - `agent_keypairs`（agent_id / public_key / encrypted_private_key）
  - `instance_keypair`（单行表：public_key / encrypted_private_key）
  - `contacts`（agent_address / profile_card_json / source / added_at / blocked）
  - `contact_requests`（from_address / to_address / status / created_at）
  - `groups`（group_id / name / created_by / created_at）
  - `group_members`（group_id / agent_address / role / joined_at）
  - `outbox`（envelope_id / envelope_json / status / retry_count / created_at）

- [ ] **Step 2:** 扩展现有表
  - `agents` 表增加：public_key, visibility, bio, tags, pricing_policy, online_status
  - `conversations` 表增加：conversation_type, remote_agent_address
  - `messages` 表增加：sender_address, encrypted, envelope_id

- [ ] **Step 3:** 索引创建 + 迁移测试
- [ ] **Step 4:** Commit

---

## Phase E: Agent 身份系统

> **目标：** 每个 Agent 拥有全局唯一加密身份。

### Task E1: clawx-identity crate

**Files:**
- Create: `crates/clawx-identity/Cargo.toml`
- Create: `crates/clawx-identity/src/lib.rs`
- Create: `crates/clawx-identity/src/keypair.rs`
- Create: `crates/clawx-identity/src/address.rs`
- Create: `crates/clawx-identity/src/profile.rs`
- Modify: `Cargo.toml`（workspace members）

- [ ] **Step 1:** 密钥对管理
  - `generate_agent_keypair()` → Ed25519 密钥对
  - `generate_instance_keypair()` → 实例级密钥对
  - 私钥加密存储（AES-256-GCM + macOS Keychain 派生密钥）
  - 公钥导出为 hex

- [ ] **Step 2:** AgentAddress 系统
  - `AgentAddress::new(pubkey_hash, instance_id)` 构造
  - 解析 `<hash>@<instance>` 字符串
  - `pubkey_hash` = SHA-256(public_key)[0..16] hex

- [ ] **Step 3:** ProfileCard 签名与验签
  - `sign_profile(card, agent_keypair)` → 签名后的名片
  - `verify_profile(signed_card, public_key)` → bool
  - 名片 JSON 序列化标准化（字段排序 + 紧凑格式）

- [ ] **Step 4:** Agent 创建时自动生成密钥对（集成到 agent_repo）
- [ ] **Step 5:** 测试：密钥生成 + 地址解析 + 名片签名验签 + 篡改检测
- [ ] **Step 6:** Commit

---

## Phase F: 通信协议层

> **目标：** 实现消息信封定义和端到端加密。

### Task F1: clawx-protocol crate

**Files:**
- Create: `crates/clawx-protocol/Cargo.toml`
- Create: `crates/clawx-protocol/src/lib.rs`
- Create: `crates/clawx-protocol/src/envelope.rs`
- Create: `crates/clawx-protocol/src/crypto.rs`
- Create: `crates/clawx-protocol/src/session.rs`
- Modify: `Cargo.toml`（workspace members + x25519-dalek + aes-gcm 依赖）

- [ ] **Step 1:** 信封序列化
  - Envelope struct（from / to / timestamp / envelope_sig / type / payload bytes）
  - 序列化为 MessagePack 或 CBOR（紧凑二进制）
  - Ed25519 信封签名（发送方 instance key 签名）
  - 验签

- [ ] **Step 2:** 1:1 加密（X25519 + AES-256-GCM）
  - `create_session(my_keypair, their_pubkey)` → SessionKey
  - `encrypt_payload(session_key, payload)` → EncryptedPayload
  - `decrypt_payload(session_key, encrypted)` → Payload
  - 每条消息使用随机 nonce

- [ ] **Step 3:** 群聊加密（Sender Key 方案）
  - 群主生成群密钥
  - 成员加入时分发（用成员公钥加密群密钥）
  - 成员变动时轮换群密钥
  - 预留接口，v0.5 完整实现

- [ ] **Step 4:** 测试：信封签名验签、加密解密往返、篡改检测、不同密钥解密失败
- [ ] **Step 5:** Commit

---

## Phase G: Relay 客户端 + 服务端 MVP

> **目标：** 实现最小可用的 Relay 中转服务。

### Task G1: clawx-relay-client crate

**Files:**
- Create: `crates/clawx-relay-client/Cargo.toml`
- Create: `crates/clawx-relay-client/src/lib.rs`
- Create: `crates/clawx-relay-client/src/connection.rs`
- Create: `crates/clawx-relay-client/src/registry.rs`

- [ ] **Step 1:** WebSocket 长连接管理
  - 连接 Relay（WSS）
  - 心跳保活（30s ping/pong）
  - 断线自动重连（指数退避，上限 60s）
  - 连接状态回调

- [ ] **Step 2:** 身份注册
  - `register_agent(profile_card)` → Relay 注册 Agent 名片
  - `update_status(online_status)` → 更新在线状态
  - `unregister_agent(agent_address)` → 注销

- [ ] **Step 3:** 消息收发
  - `send_envelope(envelope)` → 发送加密信封到 Relay
  - 接收信封 → 回调处理
  - 离线消息拉取（连接后获取缓存信封）

- [ ] **Step 4:** 测试（对接 mock Relay server）
- [ ] **Step 5:** Commit

### Task G2: clawx-relay-server（独立二进制）

**Files:**
- Create: `apps/clawx-relay/Cargo.toml`
- Create: `apps/clawx-relay/src/main.rs`
- Create: `apps/clawx-relay/src/registry.rs`
- Create: `apps/clawx-relay/src/router.rs`
- Create: `apps/clawx-relay/src/store.rs`

- [ ] **Step 1:** WebSocket 服务端
  - Axum + `axum::extract::ws` WebSocket 支持
  - 连接管理（instance_id → WebSocket sender）
  - 心跳检测（超时断开）

- [ ] **Step 2:** 身份注册表
  - 内存注册表：AgentAddress → ProfileCard + 在线状态
  - 公开 Agent 搜索接口（`GET /agents?q=keyword`）
  - Agent 名片注册 / 更新 / 注销

- [ ] **Step 3:** 消息路由
  - 收到信封 → 查找目标 instance 连接 → 转发
  - 目标离线 → 存入离线消息队列（SQLite，TTL 7 天）
  - 目标上线时推送离线消息

- [ ] **Step 4:** 端到端测试：两个 relay-client 实例 → Relay → 互发消息
- [ ] **Step 5:** Commit

---

## Phase H: 社交功能

> **目标：** 通讯录管理 + 添加好友 + 远程 1:1 对话。

### Task H1: clawx-social crate

**Files:**
- Create: `crates/clawx-social/Cargo.toml`
- Create: `crates/clawx-social/src/lib.rs`
- Create: `crates/clawx-social/src/contacts.rs`
- Create: `crates/clawx-social/src/friend_request.rs`

- [ ] **Step 1:** 通讯录管理
  - SqliteContactRegistry：add / get / list / update / delete / block / unblock
  - 名片缓存（远程 Agent 名片存储在本地）
  - 名片定期更新机制

- [ ] **Step 2:** 添加好友流程
  - 发送好友请求（构造 ContactRequest 信封 → Relay 转发）
  - 接收好友请求（回调 → 自动接受 / 手动审核）
  - 接受后双方交换公钥 + 建立 SessionKey
  - 拒绝 / 屏蔽

- [ ] **Step 3:** 测试：完整好友添加流程
- [ ] **Step 4:** Commit

### Task H2: 远程 1:1 对话

**Files:**
- Modify: `crates/clawx-runtime/src/agent_loop.rs`
- Modify: `crates/clawx-runtime/src/dispatcher.rs`
- Modify: `crates/clawx-api/src/routes/conversations.rs`

- [ ] **Step 1:** Runtime 支持远程消息
  - Dispatcher 识别 remote conversation → 通过 RelayClient 发送
  - 接收远程消息 → 创建/续接 Conversation → Agent Loop 处理 → 回复发送到 Relay

- [ ] **Step 2:** 对话类型扩展
  - ConversationType::Remote 支持
  - 远程对话创建：指定目标 AgentAddress → 建立加密会话 → 发消息

- [ ] **Step 3:** API 端点
  - `POST /contacts` — 添加好友
  - `GET /contacts` — 通讯录列表
  - `POST /contacts/:address/accept` / `reject`
  - `POST /conversations` 支持 `remote_agent_address` 参数

- [ ] **Step 4:** 端到端测试：两台 Mac 各跑 ClawX → 互加好友 → E2E 加密对话
- [ ] **Step 5:** Commit

---

## Phase I: GUI 社交页面 + v0.3 验收

> **目标：** GUI 支持远程对话和通讯录，完成 v0.3 验收。

### Task I1: 通讯录页扩展

**Files:**
- Modify: `apps/clawx-gui/src/pages/ContactsPage.tsx`

- [ ] **Step 1:** B 栏通讯录扩展
  - 分组：「我的 Agent」+「远程好友」
  - 远程好友显示：名片信息 + 在线状态
  - 「+ 添加好友」按钮（输入 AgentAddress / 扫码 / 邀请链接）

- [ ] **Step 2:** C 栏远程 Agent 名片
  - 名片展示（名称/简介/标签/收费策略/在线状态）
  - 「发消息」按钮 → 跳转到对话页
  - 「屏蔽」按钮

- [ ] **Step 3:** 好友请求通知
  - 新请求提示（通讯录图标红点）
  - 请求列表（接受/拒绝按钮）

- [ ] **Step 4:** Commit

### Task I2: 远程对话 UI

**Files:**
- Modify: `apps/clawx-gui/src/pages/ChatPage.tsx`

- [ ] **Step 1:** 对话列表区分本地/远程
  - 远程对话标记（如小地球图标）
  - 加密状态指示（锁图标）

- [ ] **Step 2:** 远程对话体验
  - 发送消息 → 加密 → Relay 路由 → 对方 Agent 处理 → 加密回复 → 解密显示
  - 对方离线时提示
  - 消息发送状态（发送中/已送达/失败）

- [ ] **Step 3:** Commit

### Task I3: v0.3 验收

- [ ] **Step 1:** 验收场景：两台 Mac 各运行 ClawX + Relay
  - Mac A 创建 Agent 并设为 public
  - Mac B 通过 Relay 搜索到 Agent → 添加好友
  - Mac A 接受好友请求
  - Mac B 与 Mac A 的 Agent 进行端到端加密对话
  - 验证 Relay 无法读取对话内容

- [ ] **Step 2:** 性能验收
  - 远程消息端到端延迟 < 2s（同网段）
  - 断线重连 < 5s
  - 离线消息送达（目标上线后 < 10s）

- [ ] **Step 3:** 生成 v0.3 验收报告
- [ ] **Step 4:** Commit

---

## 关键风险

| 风险 | 等级 | 应对 |
|------|------|------|
| GUI 开发周期长 | 高 | Phase B 可按页面并行开发，优先对话页和设置页 |
| Telegram/Lark API 变动 | 中 | Adapter 抽象层隔离，保持 trait 稳定 |
| X25519 密钥协商复杂度 | 中 | 先实现 1:1，群密钥协商延后到 v0.5 |
| Relay 单点故障 | 中 | MVP 先单实例，后续支持多 Relay 配置 |
| 跨设备 NAT 穿透 | 低 | Relay 兜底转发，不依赖直连 |

---

## 预估工作量

| Phase | 内容 | 预估复杂度 |
|-------|------|-----------|
| A | GUI 基础架构 | 中 |
| B | GUI 6 个核心页面 | 高（最大工作量） |
| C | 真实集成 + 验收 | 中 |
| D | v0.3 类型 + DB | 中 |
| E | Identity crate | 中 |
| F | Protocol crate | 高（加密复杂） |
| G | Relay client + server | 高 |
| H | Social crate + Runtime 集成 | 高 |
| I | GUI 社交 + 验收 | 中 |
