# ClawX Architecture v3.0

**版本:** 4.0
**日期:** 2026年3月18日
**对应 PRD:** v2.0
**变更说明:** 基于 IronClaw/OpenFang 安全实践升级安全架构至 12 层；新增智能模型路由、MCP 协议支持、预算追踪

---

## 1. 架构概览

ClawX 采用**分层单体 + 模块化 Crate** 架构，以 Rust Workspace 为核心组织形式。整体架构遵循以下设计原则：

- **本地优先 (Local-First)**：所有核心能力在本地闭环运行，无需云端依赖
- **Trait 驱动 (Trait-Driven)**：所有后端实现通过 Trait 抽象，支持可插拔替换
- **安全纵深 (Defense-in-Depth)**：12 层纵深防御体系，从 T1 双计量沙箱到 T3 原生操作的分级执行模型
- **事件驱动 (Event-Driven)**：模块间通过 EventBus 解耦通信（v0.1 先用 Trait 直调，EventBus 保留架构位但延后实现）

### 1.1 系统全局架构图

```
┌─────────────────────────────────────────────────────────────────────┐
│                        Presentation Layer                           │
│  ┌──────────┐ ┌──────────┐ ┌──────────────────┐ ┌──────────────┐  │
│  │ SwiftUI  │ │   CLI    │ │ IM Channels      │ │ iOS App      │  │
│  │ GUI      │ │ (clawx)  │ │ (Lark/TG/Slack…) │ │ (via Relay)  │  │
│  └────┬─────┘ └────┬─────┘ └────────┬─────────┘ └──────┬───────┘  │
│       │ FFI        │ direct         │                   │ Cloud    │
└───────┼────────────┼────────────────┼───────────────────┼──────────┘
        ▼            ▼                ▼                   ▼
                                                 ┌──────────────────┐
                                                 │ Cloud Relay (WSS)│
                                                 │ E2E 加密转发      │
                                                 └────────┬─────────┘
                                                          │
┌─────────────────────────────────────────────────────────────────────┐
│                         API / Gateway Layer                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  clawx-api   │  │ clawx-gateway│  │      clawx-ffi           │  │
│  │  (REST/Axum) │  │ (路由/代理)   │  │  (SwiftUI ↔ Rust 桥接)   │  │
│  └──────┬───────┘  └──────┬───────┘  └────────────┬─────────────┘  │
└─────────┼──────────────────┼──────────────────────┼────────────────┘
          ▼                  ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         API / Gateway Layer                         │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────────────────┐  │
│  │  clawx-api   │  │ clawx-gateway│  │      clawx-ffi           │  │
│  │  (REST/Axum) │  │ (路由/代理)   │  │  (SwiftUI ↔ Rust 桥接)   │  │
│  └──────┬───────┘  └──────┬───────┘  └────────────┬─────────────┘  │
└─────────┼──────────────────┼──────────────────────┼────────────────┘
          ▼                  ▼                      ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        Core Runtime Layer                           │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                     clawx-runtime                            │   │
│  │  Agent 生命周期管理 │ 对话编排 │ Tool 调度 │ 上下文管理        │   │
│  └──────────────────────────────────────────────────────────────┘   │
│                                                                     │
│  ┌──────────────────────────────────────────────────────────────┐   │
│  │                     clawx-eventbus                           │   │
│  │  异步事件总线 │ 发布/订阅 │ 模块间解耦通信                     │   │
│  └──────────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Domain Services Layer                         │
│                                                                     │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │
│  │  clawx-llm  │ │clawx-memory │ │  clawx-kb   │ │clawx-security│  │
│  │ LLM 多模型  │ │ 两层记忆     │ │ 知识库引擎  │ │ 安全执行官   │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘  │
│                                                                     │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │
│  │clawx-skills │ │clawx-schedu.│ │clawx-channel│ │clawx-artifact│  │
│  │ Skills 引擎 │ │ 定时/事件   │ │ IM 渠道管理 │ │ 产物管理     │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       Infrastructure Layer                          │
│                                                                     │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐ ┌─────────────┐  │
│  │ clawx-vault │ │ clawx-hal   │ │clawx-daemon │ │  clawx-ota  │  │
│  │ 数据保险箱  │ │ 硬件抽象层  │ │ 进程守护    │ │  OTA 更新   │  │
│  └─────────────┘ └─────────────┘ └─────────────┘ └─────────────┘  │
│                                                                     │
│  ┌─────────────┐ ┌─────────────┐                                   │
│  │clawx-config │ │ clawx-types │                                   │
│  │ 配置管理    │ │ 共享类型    │                                   │
│  └─────────────┘ └─────────────┘                                   │
└─────────────────────────────────────────────────────────────────────┘
          │
          ▼
┌─────────────────────────────────────────────────────────────────────┐
│                       External Dependencies                         │
│  SQLite │ Qdrant (embedded) │ Tantivy │ Wasmtime │ macOS APIs      │
└─────────────────────────────────────────────────────────────────────┘
```

### 1.2 技术栈选型

| 层次 | 技术 | 选型理由 |
|------|------|---------|
| 核心运行时 | Rust + tokio | 高性能、内存安全、零成本抽象 |
| GUI | SwiftUI (macOS) | 原生体验、系统深度集成 |
| 数据库 | SQLite (sqlx) | 本地优先、零运维、嵌入式 |
| 向量数据库 | Qdrant (embedded) | 嵌入式模式、高性能向量检索 |
| 全文检索 | Tantivy (BM25) | Rust 原生、高性能全文索引 |
| WASM 沙箱 | Wasmtime (wasmtime-wasi) + 双计量 | 安全沙箱执行、权限隔离、燃料+纪元双计量防 DoS |
| 密钥安全 | zeroize crate | 密钥使用后自动从内存擦除 |
| 泄漏检测 | aho-corasick crate | O(n) 多模式匹配，DLP 扫描优化 |
| HTTP 框架 | Axum | Tokio 生态原生、类型安全路由 |
| HTTP 客户端 | Reqwest | 异步 HTTP、TLS 支持 |
| 序列化 | serde + serde_json + toml | Rust 生态标准 |
| 日志 | tracing + tracing-subscriber | 结构化日志、性能追踪 |
| FFI 桥接 | swift-bridge / uniffi | SwiftUI ↔ Rust 双向通信 |
| 移动端通信 | Cloud Relay (WSS) + E2E 加密 (X25519) | 云端转发，Relay 不可解密 |

---

## 2. 模块职责与边界

### 2.1 Foundation Layer (基础层)

#### clawx-types
- **职责**：全局共享的类型定义、错误类型、Trait 接口
- **依赖**：仅依赖 serde/chrono/uuid/thiserror/async-trait
- **设计原则**：零业务逻辑，纯粹的类型与 Trait 定义
- **核心内容**：
  - `AgentId`, `AgentConfig`, `AgentStatus` — Agent 核心类型
  - `Memory`, `MemoryType`, `MemoryScope` — 记忆类型
  - `Message`, `Conversation` — 对话类型
  - `ClawxError` — 统一错误枚举
  - `Result<T>` — 全局 Result 别名
  - 核心 Trait 接口（`LlmProvider`, `MemoryStore`, `KnowledgeStore` 等）

#### clawx-config
- **职责**：配置文件加载、验证、热更新
- **依赖**：clawx-types
- **配置文件格式**：TOML (`~/.clawx/config.toml`)
- **核心内容**：
  - 全局配置、Agent 配置、LLM Provider 配置
  - 安全策略配置、网络白名单配置
  - 配置变更通知（通过 EventBus）

### 2.2 Core Runtime Layer (核心运行层)

#### clawx-eventbus
- **职责**：异步事件发布/订阅总线，模块间解耦通信
- **依赖**：clawx-types
- **设计模式**：Pub/Sub with topic-based routing
- **实施策略**：v0.1 阶段模块间使用 Trait 直接调用；当 1:N 广播场景（如"文件删除"需同时通知 Vault、Security、Artifact）频繁出现后，启用 EventBus 替换直调
- **核心事件类别**：
  - `agent.*` — Agent 生命周期事件
  - `memory.*` — 记忆变更事件
  - `security.*` — 安全告警事件
  - `vault.*` — 版本点创建/回滚事件
  - `channel.*` — IM 渠道消息事件
  - `scheduler.*` — 定时任务触发事件

#### clawx-runtime
- **职责**：Agent 运行时引擎，核心调度器
- **依赖**：clawx-types, clawx-llm, clawx-memory, clawx-security, clawx-eventbus, clawx-vault
- **核心能力**：
  - Agent 实例生命周期管理（创建、启动、暂停、销毁）
  - 对话上下文管理与窗口压缩
  - Tool/Capability 调度与执行
  - LLM 请求编排（记忆注入 → 知识检索 → Prompt 组装 → LLM 调用 → 安全扫描）
  - 多 Agent 并发调度（建议活跃 ≤ 3）

### 2.3 Domain Services Layer (领域服务层)

#### clawx-llm
- **职责**：LLM Provider 统一抽象、多模型管理与智能路由
- **依赖**：clawx-types, clawx-config
- **Trait 接口**：`LlmProvider`, `ModelRouter`
- **支持的 Provider**：
  - OpenAI API (GPT-4o, o1, etc.)
  - Anthropic API (Claude Opus/Sonnet/Haiku)
  - Google Gemini API
  - 本地模型 (Ollama / llama.cpp / vLLM)
  - 自定义 OpenAI 兼容 API
  - 中国市场: 智谱 GLM、月之暗面 Kimi、通义千问 Qwen（通过 OpenAI 兼容接口）
- **核心能力**：
  - 流式输出、Token 计数、用量统计、API Key 轮换
  - **智能模型路由 (Smart Model Routing)**：参考 IronClaw 设计，根据请求复杂度自动选择模型层级
  - **预算追踪 (Budget Tracking)**：Per-Agent 预算上限，Token 消耗累计与告警
  - **自动回退 (Auto Fallback)**：Provider 不可用时自动切换到备选模型
  - **MCP 工具集成 (Model Context Protocol)**：支持 MCP 标准协议的外部工具服务器接入
- **智能路由模型**：

  ```
  用户请求
      │
      ▼
  ┌──────────────────────────────┐
  │  复杂度评估器 (多维度评分)    │
  │                              │
  │  维度: 上下文长度、指令复杂度 │
  │  推理深度、专业领域、多模态   │
  │  历史成功率                   │
  └──────────┬───────────────────┘
             │ 评分
             ▼
  ┌──────────────────────────────┐
  │  模型层级映射                 │
  │                              │
  │  Flash (低复杂度) → Haiku    │
  │  Standard (中等)  → Sonnet   │
  │  Pro (高复杂度)   → Opus     │
  └──────────────────────────────┘
  ```

  **级联模式 (Cascade)**：可选先用低成本模型尝试，若置信度不足再升级到高层模型。用户可在 Per-Agent 配置中选择固定模型或启用智能路由。

#### clawx-memory
- **职责**：三层记忆系统——Working Memory + Short-Term Memory + Long-Term Memory
- **依赖**：clawx-types, clawx-llm, clawx-eventbus
- **存储后端**：内存 (Working/Short-Term) + SQLite (Long-Term 主存储) + Qdrant (语义检索加速)
- **三层架构**：
  - **Working Memory**：当前对话的上下文窗口管理，自动摘要压缩
  - **Short-Term Memory**：Session 级缓冲，跨对话信息延续，自动晋升评估
  - **Long-Term Memory**：持久化存储，分为 Agent Memory（私有）和 User Memory（全局共享）
- **核心能力**：
  - LLM 辅助记忆提取（隐式从对话中提取 + 显式用户告知）
  - 基于余弦相似度的语义召回 + 重要性/鲜活度加权排序
  - 艾宾浩斯遗忘曲线衰减（按重要性动态调整衰减速率）
  - 记忆主动提取与 Prompt 注入（异步提取，不阻塞响应）
  - 记忆合并与去重（定期聚类，LLM 辅助合并）
  - 共享记忆审计追溯（来源 Agent、变更历史）
- **详细设计**：见 [memory-architecture.md](./memory-architecture.md)

#### clawx-kb
- **职责**：高性能知识库引擎
- **依赖**：clawx-types, clawx-llm, clawx-eventbus
- **核心能力**：
  - FSEvents 文件夹监控 + 增量索引
  - 多格式解析（PDF/DOCX/PPTX/图片/音视频）
  - 语义分块（512 Token, 10% 重叠）
  - 混合检索：向量搜索 (Qdrant) + BM25 (Tantivy) + RRF 融合
  - 本地 Embedding（nomic-embed-text / bge-m3 / CLIP）
- **性能目标**：P50 < 800ms, P95 < 2s (10k chunks, M2/16GB)

#### clawx-security
- **职责**：安全执行官——12 层纵深防御系统
- **依赖**：clawx-types, clawx-config, clawx-eventbus
- **分级执行模型**：
  - T1 Sandboxed：WASM 双计量沙箱（燃料 + 纪元中断，无文件/网络/密钥，每次调用全新 Store）
  - T2 Restricted Process：受限子进程 + 工作区隔离 + 命令白名单 + 环境变量清理
  - T3 Native：原生能力，逐次确认
- **12 层安全能力**：
  - L1: Prompt 注入防御（三层：模式匹配 → 内容净化 → LLM 自检）
  - L2: WASM 双计量沙箱（燃料计量 + 纪元中断防 DoS）
  - L3: 宿主边界凭证注入（密钥永不进入沙箱，参考 IronClaw）
  - L4: 声明式权限能力模型（`capabilities.toml`）
  - L5: DLP 数据防泄漏 + Aho-Corasick 多模式泄漏检测
  - L6: SSRF 防护（私有 IP/云元数据/DNS 重绑定）+ 网络白名单 + 防火墙
  - L7: 路径穿越防护（规范化 + 符号链接检查）
  - L8: 密钥零化（`Zeroizing<String>`，使用后立即从内存擦除）
  - L9: 循环守卫（Agent 调用链哈希检测乒乓模式）+ 子进程沙箱强化
  - L10: Skill/Agent 签名验证（Ed25519）
  - L11: GCRA 速率限制（API/LLM/工具/渠道多维度限速）
  - L12: 哈希链审计日志 + 健康端点脱敏
- **详细设计**：见 [security-architecture.md](./security-architecture.md)

#### clawx-skills
- **职责**：Skills 执行引擎、MCP 客户端与生命周期管理
- **依赖**：clawx-types, clawx-security, clawx-eventbus
- **核心能力**：
  - Skill 安装/卸载/启用/停用
  - WASM 沙箱内 Skill 执行（每次调用全新 Store，宿主边界凭证注入）
  - Skill 权限声明（`capabilities.toml`）与运行时校验
  - Skill Ed25519 签名验证（防供应链攻击）
  - Skill 版本管理
  - **MCP 客户端**：连接外部 MCP 工具服务器，扩展 Agent 可用工具集

#### clawx-scheduler
- **职责**：主动式 Agent 的调度引擎
- **依赖**：clawx-types, clawx-eventbus
- **核心能力**：
  - Cron 表达式定时任务
  - 系统事件驱动触发
  - 自然语言 → Cron 转换
  - 任务持久化（不依赖 GUI）

#### clawx-channel
- **职责**：IM 渠道统一接入层
- **依赖**：clawx-types, clawx-eventbus
- **支持渠道**：飞书/Lark, Telegram, Slack, WhatsApp, Discord, 企业微信
- **Trait 接口**：`ChannelAdapter`
- **核心能力**：
  - 渠道生命周期管理（连接/断连/重连）
  - 消息格式统一化
  - 会话隔离（DM / Group 独立上下文）
  - 主动推送能力

#### clawx-artifact
- **职责**：Agent 生成文件的统一管理
- **依赖**：clawx-types, clawx-eventbus
- **核心能力**：
  - 文件索引、分类、来源追溯
  - 预览支持（PDF/图片/代码）
  - 导出与分享

### 2.4 Infrastructure Layer (基础设施层)

#### clawx-vault
- **职责**：数据保险箱——工作区版本化与回滚
- **依赖**：clawx-types, clawx-eventbus
- **核心能力**：
  - 自动版本点创建（Agent 操作前触发）
  - 变更集记录（新增/修改/删除文件清单）
  - 差异预览与文件级/任务级回滚
  - 智能清理（7天全保留 → 30天每天1个 → 过期删除）
  - 工作区边界控制（仅保障 ClawX Workspace 内文件）

#### clawx-hal
- **职责**：macOS 硬件抽象层
- **依赖**：clawx-types
- **核心能力**：
  - macOS 系统 API 封装（FSEvents, Keychain, Notification, pf）
  - Apple Silicon / Intel 适配
  - 设备发现（摄像头、智能家居）
  - 系统资源监控（CPU/内存/磁盘）

#### clawx-daemon
- **职责**：macOS launchd 集成与进程内健康自检（**不是**独立守护进程，而是 clawx-service 内的模块）
- **依赖**：clawx-types, clawx-eventbus, clawx-runtime
- **核心能力**：
  - **launchd 集成**：生成并注册 `com.clawx.service.plist`（Launch Agent），由 macOS launchd 负责开机自启和崩溃重启（< 5s）
  - **进程内健康自检**：心跳检测各模块状态、内存泄漏趋势监控（超阈值主动触发优雅重启）
  - **崩溃恢复**：进程重启后从 SQLite 恢复中断的任务队列和 Agent 状态
  - **优雅关闭**：收到 SIGTERM 时保存内存状态、刷写日志、通知各模块清理

#### clawx-ota
- **职责**：OTA 远程更新机制
- **依赖**：clawx-types, clawx-hal
- **核心能力**：
  - 自动检查更新、增量 Delta 更新
  - Ed25519 签名验证
  - 更新日志展示

### 2.5 API / Gateway Layer (接口层)

#### clawx-api
- **职责**：REST API 服务，对外提供 HTTP 接口
- **依赖**：clawx-types, clawx-runtime, clawx-gateway
- **框架**：Axum
- **核心接口**：
  - `/agents` — Agent CRUD
  - `/conversations` — 对话管理
  - `/memory` — 记忆读写
  - `/knowledge` — 知识库检索
  - `/tasks` — 定时任务管理
  - `/system` — 系统状态与健康检查

#### clawx-gateway
- **职责**：请求路由与渠道消息分发
- **依赖**：clawx-types, clawx-channel
- **核心能力**：
  - IM 消息 → Agent 路由
  - 渠道消息格式统一化

#### clawx-ffi
- **职责**：SwiftUI ↔ Rust FFI 桥接层
- **依赖**：clawx-types, clawx-runtime
- **桥接方案**：swift-bridge 或 uniffi
- **设计原则**：保持 FFI 边界薄，仅传递简单类型

### 2.6 Application Layer (应用层)

#### clawx-service (apps/)
- **职责**：后台守护服务主进程
- **依赖**：clawx-types, clawx-runtime, clawx-config, clawx-daemon, clawx-eventbus
- **运行方式**：macOS Launch Agent，开机自启动

#### clawx-cli (apps/)
- **职责**：命令行交互工具
- **依赖**：clawx-types, clawx-runtime, clawx-config

---

## 3. 核心数据流

### 3.1 用户对话请求流程

```
User Input (GUI/CLI/IM)
    │
    ▼
┌─────────┐    ┌───────────┐    ┌──────────────┐
│ clawx-  │───▶│ clawx-    │───▶│ clawx-       │
│ api/ffi │    │ runtime   │    │ security     │
└─────────┘    │           │    │ (权限检查)    │
               │           │    └──────┬───────┘
               │           │           │ ✓
               │           │◀──────────┘
               │           │
               │  ┌────────┴────────┐
               │  │ 并行检索        │
               │  ▼                 ▼
               │ clawx-memory  clawx-kb
               │ (记忆召回)    (知识检索)
               │  │                 │
               │  └────────┬────────┘
               │           ▼
               │  Prompt 组装 (System + Memory + Knowledge + User Input)
               │           │
               │           ▼
               │      clawx-llm
               │      (LLM 调用, 流式输出)
               │           │
               │           ▼
               │  clawx-security (DLP 出站扫描)
               │           │
               │           ▼
               │  Response → User
               └───────────────────
```

### 3.2 主动式 Agent 执行流程

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│ clawx-       │────▶│ clawx-       │────▶│ clawx-       │
│ scheduler    │     │ eventbus     │     │ runtime      │
│ (Cron 触发)  │     │ (事件分发)   │     │ (Agent 执行) │
└──────────────┘     └──────────────┘     └──────┬───────┘
                                                  │
                                                  ▼
                                          ┌──────────────┐
                                          │ clawx-channel│
                                          │ (结果推送)    │
                                          │ Lark/TG/Slack│
                                          └──────────────┘
```

### 3.3 知识库索引流程

```
文件系统变更
    │ FSEvents (clawx-hal)
    ▼
┌──────────────┐
│  clawx-kb    │
│  1. 文件解析 │ (PDF/DOCX/图片/音视频)
│  2. 语义分块 │ (512 Token, 10% 重叠)
│  3. Embedding│ (nomic-embed-text / CLIP)
│  4. 写入索引 │
│     ├─ Qdrant (向量索引)
│     └─ Tantivy (BM25 索引)
└──────────────┘
```

### 3.4 工作区回滚流程

```
Agent 执行文件操作
    │
    ▼
┌──────────────┐     ┌──────────────┐
│ clawx-       │────▶│ clawx-vault  │
│ runtime      │     │ 1. 创建版本点│
│ (操作前拦截) │     │ 2. 记录变更集│
└──────────────┘     └──────────────┘
                            │
                     用户请求回滚
                            │
                            ▼
                     ┌──────────────┐
                     │ clawx-vault  │
                     │ 1. 差异预览  │
                     │ 2. 文件级还原│
                     │ 3. 任务级回滚│
                     └──────────────┘
```

---

## 4. 部署架构

### 4.1 进程模型

ClawX 在 macOS 上以**双进程模型**运行，进程守护由 macOS launchd 提供：

```
┌─────────────────────────────────────────────────────────────────┐
│  macOS launchd (操作系统级进程管理器)                            │
│                                                                  │
│  ~/Library/LaunchAgents/com.clawx.service.plist                  │
│    KeepAlive: true          ← 崩溃自动重启 (< 5s)               │
│    RunAtLoad: true          ← 开机自动启动                       │
│    ProcessType: Background  ← 后台低优先级运行                   │
│                                                                  │
│  launchd 负责: 启动 → 监控 → 崩溃重启 → 资源回收                │
└──────────────────────────┬──────────────────────────────────────┘
                           │ 管理
                           ▼
┌──────────────────────────────────────┐
│  clawx-service (后台进程，无 UI)     │
│  用户不可见，无 Dock 图标             │
│                                      │
│  ┌────────────────────────────────┐  │
│  │  Runtime Engine (Agent 调度)   │  │
│  │  Scheduler Engine (定时任务)   │  │
│  │  API Server (127.0.0.1:19200) │  │
│  │  KB Engine (后台索引)          │  │
│  │  Channel Listener (IM 监听)   │  │
│  │  Daemon (健康自检 + 恢复)     │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
                           ▲
                           │ FFI / HTTP (127.0.0.1)
                           │
┌──────────────────────────────────────┐
│  ClawX.app (GUI 进程，用户可见)       │
│                                      │
│  SwiftUI Views                       │
│    Agent Workbench │ KB Browser      │
│    Memory Panel    │ Settings        │
│         │                            │
│         ▼ FFI (swift-bridge)         │
│  Embedded Rust (clawx-ffi → runtime) │
└──────────────────────────────────────┘
```

- **clawx-service**：24/7 后台进程，由 macOS launchd 守护。用户不可见（无 Dock 图标），承载所有核心逻辑
- **ClawX.app**：GUI 前端，通过 FFI 或本地 HTTP 与 service 通信
- **launchd**：操作系统级守护——开机自启、崩溃重启、资源回收。clawx-daemon 模块只负责生成 plist 和进程内健康自检
- GUI 关闭不影响后台 service 继续运行（定时任务、IM 监听等）

### 4.2 本地存储布局

```
~/.clawx/
├── config.toml              # 全局配置
├── db/
│   └── clawx.db             # SQLite 主数据库 (Agent/Memory/对话)
├── knowledge/
│   ├── qdrant/              # Qdrant 向量索引数据
│   └── tantivy/             # Tantivy BM25 索引数据
├── workspace/               # ClawX 受管工作区
│   ├── agents/              # 各 Agent 工作目录
│   └── artifacts/           # Agent 生成的产物
├── vault/                   # 版本点与变更集
├── skills/                  # 已安装的 Skills
├── audit/                   # 审计日志 (追加写入)
├── logs/                    # 运行日志
├── models/                  # 本地 Embedding 模型
└── cache/                   # 缓存 (可安全清除)
```

### 4.3 移动端远程访问架构（Cloud Relay 模式）

移动端通过**云端 Relay 转发服务**与 Mac 主机通信，Relay 服务仅做消息路由，不解密用户数据。

```
┌──────────────┐         ┌─────────────────────┐         ┌──────────────┐
│  iOS App     │◀═══════▶│  Cloud Relay Service │◀═══════▶│  Mac 主机     │
│  (SwiftUI)   │  HTTPS  │                     │  WSS    │  clawx-service│
│              │         │  • 设备注册/发现     │         │              │
│  发送指令    │         │  • 消息转发 (不解密) │         │  执行计算    │
│  接收结果    │         │  • APNs 推送代理     │         │  返回结果    │
│  接收通知    │         │  • 心跳保活          │         │  推送通知    │
│              │         │  • 离线消息缓存      │         │              │
└──────────────┘         └─────────────────────┘         └──────────────┘
       │                          │                              │
       └──────── E2E 加密 ────────┴──────── E2E 加密 ────────────┘
                  (Relay 服务不可解密)
```

**Cloud Relay 服务职责**：
- **设备注册与发现**：Mac 上线后通过 WSS 长连接注册到 Relay；iOS 通过账号发现已注册的 Mac
- **消息路由**：双向转发加密消息，不存储、不解密用户数据
- **推送代理**：Mac 主机产生通知时，通过 Relay → APNs 推送到 iOS 设备
- **离线缓存**：iOS 离线时暂存通知消息，上线后投递（TTL 7 天）
- **心跳保活**：检测 Mac 在线状态，iOS 端展示主机连接状态

**安全保障**：
- Mac 和 iOS 之间通过 X25519 密钥协商建立 E2E 加密通道
- Relay 服务仅转发密文，无法解密任何用户数据
- 依赖账号体系（同一账号下的设备才能互相发现和通信）

**依赖关系**：移动端能力依赖 v0.3+ 的账号体系，Cloud Relay 作为独立后端服务部署。

**Cloud Relay 技术栈（建议）**：

| 组件 | 技术 |
|------|------|
| 服务框架 | Rust (Axum) 或 Go |
| 长连接 | WebSocket (WSS) |
| 推送 | APNs (iOS) |
| 消息队列 | Redis (离线消息缓存) |
| 部署 | Cloudflare Workers / AWS Lambda / 自建 |

---

## 5. 安全架构

ClawX 实施 **12 层纵深防御体系**，在安全设计上对标 IronClaw（13 层管道）和 OpenFang（16 层防御），根据 ClawX 本地优先的产品特点选取了最适合的安全能力组合。

核心安全增强（v4.0 新增）：
- **WASM 双计量沙箱**：燃料计量 + 纪元中断，防止 CPU 密集型和宿主调用阻塞型 DoS
- **宿主边界凭证注入**：参考 IronClaw 设计，密钥永不进入 WASM 沙箱，仅在宿主侧 HTTP 调用边界注入
- **密钥零化**：使用 `zeroize` crate 的 `Zeroizing<String>`，密钥使用后立即从内存擦除
- **SSRF 防护**：拦截私有 IP、云元数据端点、DNS 重绑定攻击
- **Aho-Corasick 泄漏检测**：参考 IronClaw LeakDetector，O(n) 时间复杂度多模式匹配
- **Ed25519 签名验证**：Skill 包签名防供应链攻击
- **GCRA 速率限制**：多维度精确限速，适合嵌入式场景
- **循环守卫**：检测 Agent 调用链乒乓模式，防止无限循环

详见 [security-architecture.md](./security-architecture.md)

---

## 6. 阶段交付对照

| 阶段 | 核心模块 | 架构要求 |
|------|---------|---------|
| **v0.1 本地闭环** | types, config, llm(含智能路由基础), runtime, memory, kb, vault, security(L4/L5/L6/L7/L8/L11/L12 共 7 层基线), daemon(基线), ffi, api（eventbus 保留接口定义，v0.1 用 Trait 直调） | 全部本地闭环，无需网络/账号 |
| **v0.2 扩展执行** | skills, scheduler, channel, gateway, security(L1/L2/L3/L9/L10 完整 12 层), eventbus(启用实现), MCP 客户端 | 在 v0.1 基础上扩展，不反向依赖 |
| **v0.3+ 平台服务** | artifact, ota, hal(完整), 账号/同步模块, **Cloud Relay 服务**, 移动端 | 不改变本地优先默认行为；Cloud Relay 为独立后端服务 |
| **v1.0+ 生态** | HireClaw 社区、商业化 | 平台生态层，独立服务 |

---

## 7. 关键架构决策

| 决策 | 选择 | 理由 |
|------|------|------|
| 单体 vs 微服务 | 分层单体 (Rust Workspace) | 本地应用无需微服务开销，Crate 已提供足够模块化 |
| 数据库 | SQLite | 嵌入式零运维，适合本地优先架构 |
| 向量检索 | Qdrant embedded | 嵌入式模式，无需独立进程 |
| 沙箱方案 | Wasmtime WASM + 双计量 | 成熟的 WASM 运行时，燃料+纪元双计量防 DoS |
| 凭证注入 | 宿主边界注入 (参考 IronClaw) | 密钥永不进入沙箱，在 HTTP 调用边界替换占位符 |
| 密钥安全 | Zeroizing + macOS Keychain | 使用后内存擦除 + 硬件级安全存储 |
| 模型路由 | 智能路由 + 级联 (参考 IronClaw) | 按复杂度自动选择模型，降低成本 |
| GUI-Core 通信 | FFI (swift-bridge) | 最低延迟，无需 IPC 序列化开销 |
| 模块通信 | v0.1 Trait 直调 → v0.2 EventBus | v0.1 简单直接；1:N 广播场景增多后启用 EventBus |
| 进程守护 | macOS launchd (非自研守护进程) | 操作系统级守护最可靠，clawx-daemon 仅做 plist 生成和进程内自检 |
| 移动端通信 | Cloud Relay (WSS + E2E) | 云端转发比 P2P 隧道门槛低，Relay 不解密保障数据主权 |
| 配置格式 | TOML | Rust 生态标准，人类可读 |
| 日志框架 | tracing | 结构化、高性能、Span 追踪 |
