# ClawX Architecture v4.0

**日期:** 2026-03-18 | **对应 PRD:** v2.0

---

## 1. 架构概览

ClawX 采用**分层单体 + 模块化 Crate** 架构（Rust Workspace）。

**设计原则：**
- **本地优先**：所有核心能力本地闭环，无需云端依赖
- **Trait 驱动**：后端通过 Trait 抽象，可插拔替换
- **安全纵深**：12 层纵深防御，T1/T2/T3 分级执行
- **事件驱动**：模块间 EventBus 解耦（v0.1 先用 Trait 直调）

### 1.1 系统架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Presentation Layer                           │
│  SwiftUI GUI │ CLI (clawx) │ IM Channels (Lark/TG/Slack…) │ iOS   │
└───────┬────────────┬────────────────┬───────────────────┬──────────┘
        │ FFI        │ direct         │                   │ Cloud Relay
        ▼            ▼                ▼                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         API / Gateway Layer                         │
│  clawx-api (REST/Axum) │ clawx-gateway (路由) │ clawx-ffi (桥接)   │
└─────────┬──────────────────┬──────────────────────┬────────────────┘
          ▼                  ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Core Runtime Layer                           │
│  clawx-runtime: Agent 生命周期 │ 对话编排 │ Tool 调度 │ 上下文管理  │
│  clawx-eventbus: 异步事件总线 │ Pub/Sub                             │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Domain Services Layer                         │
│  clawx-llm      │ clawx-memory   │ clawx-kb       │ clawx-security│
│  clawx-skills   │ clawx-scheduler│ clawx-channel  │ clawx-artifact│
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Infrastructure Layer                          │
│  clawx-vault │ clawx-hal │ clawx-daemon │ clawx-ota               │
│  clawx-config│ clawx-types                                         │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│  External: SQLite │ Qdrant (embedded) │ Tantivy │ Wasmtime │ macOS │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 技术栈

| 层次 | 技术 | 理由 |
|------|------|------|
| 核心运行时 | Rust + tokio | 高性能、内存安全 |
| GUI | SwiftUI | 原生体验、系统深度集成 |
| 数据库 | SQLite (sqlx) | 嵌入式零运维 |
| 向量数据库 | Qdrant (embedded) | 嵌入式高性能向量检索 |
| 全文检索 | Tantivy (BM25) | Rust 原生 |
| WASM 沙箱 | Wasmtime + 双计量 | 安全隔离、防 DoS |
| HTTP | Axum + Reqwest | Tokio 原生生态 |
| FFI | swift-bridge / uniffi | SwiftUI ↔ Rust 桥接 |
| 移动端 | Cloud Relay (WSS + E2E X25519) | 云端转发，不可解密 |

---

## 2. 模块职责

### Foundation Layer

| Crate | 职责 | 核心内容 |
|-------|------|---------|
| **clawx-types** | 全局类型、Trait 接口 | AgentId, Memory, Message, ClawxError, 核心 Trait |
| **clawx-config** | 配置加载/验证/热更新 | TOML (~/.clawx/config.toml) |

### Core Runtime Layer

| Crate | 职责 |
|-------|------|
| **clawx-eventbus** | 异步 Pub/Sub 事件总线（v0.1 用 Trait 直调，v0.2 启用） |
| **clawx-runtime** | Agent 生命周期、对话编排、Tool 调度、LLM 请求编排 |

### Domain Services Layer

| Crate | 职责 | 详细设计 |
|-------|------|---------|
| **clawx-llm** | LLM 多模型管理、智能路由、预算追踪、MCP 工具集成 | — |
| **clawx-memory** | 四层记忆系统（Working/Short-Term/Long-Term/Reflection） | [memory-architecture.md](./memory-architecture.md) |
| **clawx-kb** | 知识库引擎：FSEvents 监控、多格式解析、混合检索 | — |
| **clawx-security** | 12 层纵深防御、分级执行模型 | [security-architecture.md](./security-architecture.md) |
| **clawx-skills** | Skills 执行引擎、WASM 沙箱、MCP 客户端 | — |
| **clawx-scheduler** | Cron 定时 + 事件驱动调度 | — |
| **clawx-channel** | IM 统一接入（飞书/Telegram/Slack/Discord/企业微信） | — |
| **clawx-artifact** | Agent 产物管理、预览、导出 | — |

### Infrastructure Layer

| Crate | 职责 |
|-------|------|
| **clawx-vault** | 工作区版本化与回滚（自动版本点、变更集、智能清理） |
| **clawx-hal** | macOS 硬件抽象层（FSEvents/Keychain/Notification/pf） |
| **clawx-daemon** | launchd 集成 + 进程内健康自检（非独立守护进程） |
| **clawx-ota** | OTA 远程更新、Ed25519 签名验证 |

### API / Application Layer

| Crate | 职责 |
|-------|------|
| **clawx-api** | REST API (Axum)：/agents, /conversations, /memory, /knowledge, /tasks, /system |
| **clawx-gateway** | IM 消息 → Agent 路由 |
| **clawx-ffi** | SwiftUI ↔ Rust FFI 薄层桥接 |
| **clawx-service** | 后台守护主进程（macOS Launch Agent） |
| **clawx-cli** | 命令行交互工具 |

---

## 3. 核心数据流

### 3.1 对话请求流程

```
User Input → clawx-api/ffi → clawx-runtime
  → clawx-security (权限检查)
  → 并行: clawx-memory (记忆召回) + clawx-kb (知识检索)
  → Prompt 组装 (System + Memory + Knowledge + User)
  → clawx-llm (LLM 调用, 流式输出)
  → clawx-security (DLP 出站扫描)
  → Response → User
```

### 3.2 主动式 Agent

```
clawx-scheduler (Cron) → clawx-eventbus → clawx-runtime (Agent 执行) → clawx-channel (结果推送)
```

### 3.3 知识库索引

```
FSEvents → clawx-kb: 文件解析 → 语义分块 (512T, 10%重叠) → Embedding → Qdrant + Tantivy
```

---

## 4. 部署架构

### 4.1 双进程模型

```
macOS launchd (KeepAlive + RunAtLoad, 崩溃重启 < 5s)
    │
    ▼
clawx-service (后台, 无 UI)          ClawX.app (GUI)
├── Runtime Engine                    ├── SwiftUI Views
├── Scheduler Engine                  └── FFI → Rust Core
├── API Server (127.0.0.1:19200)
├── KB Engine (后台索引)
├── Channel Listener
└── Daemon (健康自检)
```

GUI 关闭不影响后台 service 运行。

### 4.2 本地存储布局

```
~/.clawx/
├── config.toml          # 全局配置
├── db/clawx.db          # SQLite 主数据库
├── knowledge/           # Qdrant + Tantivy 索引
├── workspace/           # Agent 工作目录 + 产物
├── vault/               # 版本点与变更集
├── skills/              # 已安装 Skills
├── audit/               # 审计日志
├── models/              # 本地 Embedding 模型
└── cache/               # 可安全清除
```

### 4.3 移动端 Cloud Relay

```
iOS App ◀══ HTTPS ══▶ Cloud Relay ◀══ WSS ══▶ Mac clawx-service
                  (E2E X25519 加密, Relay 不可解密)
```

Relay 职责：设备发现、消息路由、APNs 推送代理、离线消息缓存 (TTL 7天)。依赖 v0.3+ 账号体系。

---

## 5. 智能模型路由

```
用户请求 → 复杂度评估 (上下文长度/指令复杂度/推理深度/多模态)
  → Flash (低) → Haiku | Standard (中) → Sonnet | Pro (高) → Opus
```

**级联模式**：先用低成本模型，置信度不足再升级。用户可选固定模型或智能路由。

---

## 6. 阶段交付

| 阶段 | 核心模块 |
|------|---------|
| **v0.1** | types, config, llm, runtime, memory, kb, vault, security(7层基线), daemon, ffi, api |
| **v0.2** | skills, scheduler, channel, gateway, security(完整12层), eventbus, MCP |
| **v0.3+** | artifact, ota, hal(完整), 账号/同步, Cloud Relay, 移动端 |
| **v1.0+** | HireClaw 社区、商业化 |

---

## 7. 关键架构决策（摘要）

完整 ADR 详见 [decisions.md](./decisions.md)

| 决策 | 选择 | 理由 |
|------|------|------|
| 架构风格 | 分层单体 (Rust Workspace) | 本地应用无需微服务开销 |
| 数据库 | SQLite | 嵌入式零运维 |
| 向量检索 | Qdrant embedded + Tantivy BM25 + RRF | 混合检索效果显著优于单一 |
| 沙箱 | Wasmtime WASM + 双计量 | 燃料+纪元防 DoS |
| 凭证安全 | 宿主边界注入 + Zeroizing + Keychain | 密钥永不进沙箱 |
| GUI-Core | FFI (swift-bridge) | 最低延迟 |
| 进程守护 | macOS launchd | 系统级最可靠 |
| 移动端 | Cloud Relay (WSS + E2E) | 零配置，Relay 不解密 |
