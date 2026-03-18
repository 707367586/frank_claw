# ClawX Architecture v4.1

**日期:** 2026-03-18 | **对应 PRD:** v2.0

---

## 1. 架构概览

ClawX 采用**分层单体 + 模块化 Crate** 架构（Rust Workspace）。

**设计原则：**
- **本地优先**：所有核心能力本地闭环，无需云端依赖
- **Trait 驱动**：后端通过 Trait 抽象，可插拔替换
- **安全纵深**：12 层纵深防御（L1-L12），三级执行隔离（T1 沙箱/T2 受限/T3 原生）
- **事件驱动**：模块间 EventBus 解耦（v0.1 先用 Trait 直调）

### 1.1 系统架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Presentation Layer                           │
│  SwiftUI GUI │ CLI (clawx) │ IM Channels (Lark/TG/Slack…) │ iOS   │
└───────┬────────────┬────────────────┬───────────────────┬──────────┘
        │ FFI        │ CLI            │                   │ Cloud Relay
        ▼            ▼                ▼                   ▼
┌─────────────────────────────────────────────────────────────────────┐
│             Layer 5: API / Application Layer                        │
│  clawx-ffi ─┐                                                       │
│              ├─▶ clawx-controlplane-client ──▶ clawx-api (REST/Axum)│
│  clawx-cli ─┘                           clawx-service (守护进程)   │
└─────────┬──────────────────┬──────────────────────┬────────────────┘
          ▼                  ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│             Layer 4: Core Runtime Layer                              │
│  clawx-runtime: Agent 生命周期 │ 对话编排 │ Tool 调度 │ 上下文管理  │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│             Layer 3: Services Layer                                  │
│  clawx-memory   │ clawx-kb       │ clawx-skills     │ clawx-ota   │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│             Layer 2: Domain Layer                                    │
│  clawx-llm      │ clawx-security │ clawx-vault      │ clawx-scheduler│
│  clawx-channel  │ clawx-artifact │                                  │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│             Layer 1: Config / Infrastructure Layer                   │
│  clawx-config   │ clawx-eventbus │ clawx-hal                       │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│             Layer 0: Foundation                                      │
│  clawx-types                                                        │
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

> **层级说明:** 以下按逻辑职责分组说明各 crate，精确依赖深度（Layer 0-5）详见 [crate-dependency-graph.md](./crate-dependency-graph.md)。

### Layer 0 — Foundation

| Crate | 职责 | 核心内容 |
|-------|------|---------|
| **clawx-types** | 全局类型、Trait 接口 | AgentId, Memory, Message, ClawxError, 核心 Trait |

### Layer 1 — Config / Infrastructure

| Crate | 职责 |
|-------|------|
| **clawx-config** | 配置加载/验证/热更新 (TOML ~/.clawx/config.toml) |
| **clawx-eventbus** | 异步 Pub/Sub 事件总线（v0.1 用 Trait 直调，v0.2 启用） |
| **clawx-hal** | macOS 硬件抽象层（FSEvents/Keychain/Notification/pf） |

### Layer 2 — Domain

| Crate | 职责 | 详细设计 |
|-------|------|---------|
| **clawx-llm** | LLM 多模型管理、智能路由、预算追踪、MCP 工具集成 | — |
| **clawx-security** | 12 层纵深防御、分级执行模型 | [security-architecture.md](./security-architecture.md) |
| **clawx-vault** | 工作区版本化与回滚（自动版本点、变更集、智能清理） | — |
| **clawx-scheduler** | Cron 定时 + 事件驱动调度（v0.2） | — |
| **clawx-channel** | IM 统一接入，含消息路由（v0.2） | — |
| **clawx-artifact** | Agent 产物管理、预览、导出（v0.3+） | — |

### Layer 3 — Services

| Crate | 职责 | 详细设计 |
|-------|------|---------|
| **clawx-memory** | 持久化记忆系统：v0.1 Long-Term (Agent/User Memory)，v0.2 +Short-Term；语义召回与衰减 | [memory-architecture.md](./memory-architecture.md) |
| **clawx-kb** | 知识库引擎：FSEvents 监控、多格式解析、混合检索 | — |
| **clawx-skills** | Skills 执行引擎、WASM 沙箱、MCP 客户端（v0.2） | — |
| **clawx-ota** | OTA 远程更新、Ed25519 签名验证（v0.3+） | — |

### Layer 4 — Runtime

| Crate | 职责 |
|-------|------|
| **clawx-runtime** | Agent 生命周期、对话编排、Tool 调度、LLM 请求编排、Working Memory（上下文窗口管理与压缩） |

### Layer 5 — API / Application

| Crate | 职责 |
|-------|------|
| **clawx-api** | REST API (Axum)：/agents, /conversations, /memories, /knowledge, /tasks, /system |
| **clawx-controlplane-client** | 本地控制平面客户端共享库，GUI/CLI 统一通过此访问 clawx-service |
| **clawx-ffi** | SwiftUI ↔ Rust FFI 薄层桥接，内部调用 clawx-controlplane-client |
| **clawx-service** | 后台守护主进程（macOS Launch Agent，含健康自检，由 launchd 管理生命周期） |
| **clawx-cli** | 命令行交互工具，内部调用 clawx-controlplane-client |

---

## 3. 核心数据流

### 3.1 对话请求流程

```
User Input → clawx-ffi/cli → controlplane-client → clawx-api → clawx-runtime
  → clawx-security (权限检查)
  → 并行: clawx-memory (记忆召回) + clawx-kb (知识检索)
  → Prompt 组装 (System + Memory + Knowledge + User)  [由 Runtime 完成, ADR-010]
  → clawx-llm (LLM 调用, 流式输出)
  → clawx-security (DLP 三节点扫描: LLM 出站/Tool 输出/WASM HTTP 响应)
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
├── Scheduler Engine                  └── FFI → controlplane-client → API
├── API Server (UDS ~/.clawx/run/clawx.sock)
├── KB Engine (后台索引)
├── Channel Listener
└── 健康自检 (内置于 service)
```

GUI 关闭不影响后台 service 运行。

### 4.2 本地存储布局

```
~/.clawx/
├── config.toml          # 全局配置
├── run/                 # 运行时状态 (UDS socket, control token)
│   ├── clawx.sock       # Unix Domain Socket
│   └── control.token    # 本地认证令牌
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

Relay 职责：设备发现、消息路由、APNs 推送代理、离线消息缓存 (TTL 7天)。依赖 v0.3+ 账号体系。PRD 中提及的 Tailscale/WireGuard 作为替代方案保留评估 (ADR-026)。

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
| **v0.1** | types, config, hal(基础FSEvents/Keychain), llm, runtime, memory(Long-Term), kb, vault, security(7层基线), controlplane-client, ffi, api, service, cli |
| **v0.2** | skills, scheduler, channel, security(完整12层), eventbus, MCP, memory(+Short-Term), 自主性能力(ReAct/反思/信任) |
| **v0.3+** | artifact, ota, hal(+Notification/pf完整), 账号/同步, Cloud Relay, 移动端, 多Agent协作, Computer Use |
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
| GUI/CLI-Core | controlplane-client → API (UDS) | 单入口硬边界 (ADR-003/004) |
| 进程守护 | macOS launchd（无独立 daemon 进程） | 系统级最可靠 (ADR-005) |
| 移动端 | Cloud Relay (WSS + E2E) | 零配置，Relay 不解密 |
