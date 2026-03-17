# ClawX 系统架构文档 v2.1

**版本:** 2.1
**日期:** 2026年3月17日
**基于:** PRD v2.0 + OpenClaw/IronClaw/ZeroClaw/OpenFang 竞品深度分析

> 详细竞品分析见 `docs/arch/competitive-analysis.md`

---

## 1. 架构概览

ClawX 是本地优先的 Agent Computer 平台，采用 Rust 核心 + SwiftUI 原生 GUI。本文档定义模块边界、接口、数据流和跨切面关注点。

架构融合了四大竞品的核心精华：
- **OpenClaw**：Gateway Lane 队列 + Binding Rules 路由 + 8 个生命周期 Hook + Memory Flush
- **ZeroClaw**：5 核心 Trait 驱动 + 编译时分发 + ChannelMessage 归一化 + 工作区隔离
- **IronClaw**：宿主边界凭证注入 + seL4 式 Capability 模型 + WIT 工具接口 + 双向泄漏扫描
- **OpenFang**：Kernel/Runtime 分离 + Hands 自治包 + WASM 双重计量 + Merkle 审计链 + 污点跟踪

### 1.1 关键架构决策

| 决策 | 理由 | 来源 |
|------|------|------|
| **AD-01**: 事件总线是 v0.1 交付物 | 所有模块依赖解耦通信，延迟会造成硬耦合 | 原始分析 |
| **AD-02**: 三层执行模型 (WASM/子进程/原生) | 纯 WASM 无法支持 Shell 和 Python 执行 | 原始分析 |
| **AD-03**: 懒加载组件初始化 | Qdrant、Tantivy、WASM runtime 首次使用时加载，非启动时 | ZeroClaw |
| **AD-04**: 独立 `clawx-llm` crate | Runtime、Memory、KB 都需要 LLM 访问，共享 crate 防止重复 | 原始分析 |
| **AD-05**: `clawx-types` 共享类型 crate | 防止循环依赖和类型定义重复 | OpenFang |
| **AD-06**: SQLite WAL 模式 + 每模块独立数据库 | 减少多 Agent 并发写入竞争 | 原始分析 |
| **AD-07**: v0.1 包含基础安全 (DLP + 路径限制) | 安全不是事后附加——OpenClaw 的教训 | IronClaw + OpenClaw 教训 |
| **AD-08**: UniFFI 作为 Rust-Swift FFI | 自动生成 Swift 绑定，减少手动维护 | 原始分析 |
| **AD-09**: ONNX Runtime 作为 AI 推理 HAL | 业界事实标准，同一 API 支持 CPU/GPU/NPU/vGPU | 原始分析 |
| **AD-10**: 宿主边界凭证注入 | LLM 和工具代码永远看不到密钥，彻底消除 Prompt 注入窃取凭证攻击面 | IronClaw |
| **AD-11**: Merkle hash-chain 审计日志 | 每条记录链接前一条，篡改一条则全链断裂，真正不可篡改 | OpenFang |
| **AD-12**: WASM 双重计量 (Fuel + Epoch) | Fuel 计量限制计算量 + Epoch 中断限制时间 + 看门狗线程杀死失控代码 | OpenFang |
| **AD-13**: Agent Loop 循环检测 + 熔断 | SHA256 对 tool-call 去重，检测乒乓模式并熔断 | OpenFang |
| **AD-14**: seL4 式 Capability 权限模型 | 零访问默认 + 显式能力授予，替代简单权限列表 | IronClaw |
| **AD-15**: Hands 自治 Agent 包 | TOML 清单 + 多阶段操作手册 + 定时调度，实现"主动式 Agent" | OpenFang |
| **AD-16**: Gateway Lane 队列 + Binding Rules | 每会话序列化执行防竞态 + 4 级确定性路由 | OpenClaw |

---

## 2. Rust Workspace Crate 结构

```
clawx/
  Cargo.toml                    (workspace root)
  crates/
    clawx-types/                共享类型、错误分类、事件定义、Observer trait
    clawx-config/               配置加载、验证、Schema
    clawx-eventbus/             发布-订阅事件总线
    clawx-llm/                  LLM Provider 抽象 (trait + 多 Provider 实现)
    clawx-runtime/              Agent Loop + Hook 系统 + 循环检测 + 任务分发
    clawx-memory/               三层记忆 + Memory Flush + 智能注入
    clawx-kb/                   知识库引擎 (embedding + 混合检索 + MMR + 时间衰减)
    clawx-security/             8 层纵深防御:
                                  - Capability 模型 (seL4 式)
                                  - 凭证注入器 (宿主边界)
                                  - DLP 双向扫描 + 污点跟踪
                                  - Merkle 审计链
                                  - Prompt 注入防御
                                  - 网络策略代理
                                  - 工作区隔离 + 路径净化
    clawx-vault/                APFS 快照、文件级还原
    clawx-skills/               Skill WASM 加载 + Fuel/Epoch 双计量 + WIT 接口
    clawx-scheduler/            Cron + 事件驱动 + Hands 自治包引擎
    clawx-channel/              IM 渠道适配器 + 消息归一化 (ZeroClaw 模式)
    clawx-gateway/              Binding Rules 4 级路由 + Lane 队列 + 认证限流
    clawx-artifact/             文件产物管理
    clawx-hal/                  硬件抽象 (ONNX Runtime EP)
    clawx-daemon/               看门狗、进程守护、launchd 集成
    clawx-ota/                  OTA 更新 (A/B 双分区 + Ed25519 签名)
    clawx-api/                  RESTful API (axum)
    clawx-ffi/                  UniFFI 桥接到 Swift
  hands/                        Hands 自治 Agent 包 (TOML + SYSTEM.md + SKILL.md)
  apps/
    clawx-gui/                  SwiftUI macOS 应用
    clawx-cli/                  CLI 工具
    clawx-service/              后台守护进程 binary
```

### 2.1 依赖关系图

```
clawx-types          (叶子节点：无内部依赖。含 Observer trait)
clawx-config         -> types
clawx-eventbus       -> types
clawx-llm            -> types, config
clawx-security       -> types, config, eventbus
clawx-memory         -> types, llm, eventbus
clawx-kb             -> types, llm, eventbus
clawx-vault          -> types, eventbus
clawx-skills         -> types, security, eventbus
clawx-scheduler      -> types, eventbus, skills (Hands 引擎)
clawx-channel        -> types, eventbus
clawx-gateway        -> types, channel, security
clawx-runtime        -> types, llm, memory, kb, security,
                        skills, eventbus, scheduler, vault, gateway
clawx-artifact       -> types, eventbus
clawx-hal            -> types
clawx-daemon         -> types, eventbus, runtime
clawx-ota            -> types, hal
clawx-api            -> runtime, gateway
clawx-ffi            -> runtime, api
```

### 2.2 核心 Trait 总览

融合 ZeroClaw 5 Trait + IronClaw 10 Trait 的精华：

| Trait | Crate | 来源 | 职责 |
|-------|-------|------|------|
| `LlmProvider` | clawx-llm | ZeroClaw Provider | LLM 调用抽象 |
| `AgentExecutor` | clawx-runtime | 原创 | Agent Loop 执行 |
| `AgentHook` | clawx-runtime | OpenClaw Hook | 8 个生命周期扩展点 |
| `SecurityGate` | clawx-security | IronClaw | 授权 + DLP + 审计 |
| `CredentialInjector` | clawx-security | IronClaw | 宿主边界凭证注入 |
| `MemoryStore` | clawx-memory | ZeroClaw Memory | 三层记忆 + Flush + 智能注入 |
| `KnowledgeEngine` | clawx-kb | 原创 | 混合检索 + MMR + 时间衰减 |
| `ChannelAdapter` | clawx-channel | ZeroClaw Channel | 渠道适配 + 消息归一化 |
| `Gateway` | clawx-gateway | OpenClaw Gateway | Binding Rules + Lane 队列 |
| `EventBus` | clawx-eventbus | 原创 | 发布-订阅解耦 |
| `Observer` | clawx-types | IronClaw Observer | 可插拔系统观测 |
| `AcceleratorProvider` | clawx-hal | 原创 | AI 推理硬件抽象 |
| `StorageProvider` | clawx-hal | 原创 | 存储操作抽象 |

---

## 3. 核心 Trait 定义

### 3.1 LLM Provider (`clawx-llm`)

```rust
pub struct LlmResponse {
    pub id: String,
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
    pub usage: TokenUsage,
    pub stop_reason: StopReason,
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

pub enum StopReason { EndTurn, ToolUse, MaxTokens, StopSequence }

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, request: &CompletionRequest) -> Result<LlmResponse>;
    async fn complete_stream(
        &self, request: &CompletionRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<LlmStreamChunk>> + Send>>>;
    fn metadata(&self) -> ProviderMetadata;
}
```

### 3.2 Agent Loop (`clawx-runtime`)

```rust
pub struct LoopConfig {
    pub max_iterations: u32,              // 默认: 20
    pub max_token_budget: u64,            // 总 token 预算
    pub tool_timeout: Duration,           // 单工具超时
    pub require_confirmation: Vec<String>, // 需要用户确认的工具名
}

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn run(
        &self,
        agent: &AgentConfig,
        input: UserMessage,
        context: &mut ConversationContext,
    ) -> Result<AgentResponse>;
}
```

**Agent Loop 状态机（含循环检测 + 8 个 Hook 点）：**

```
              [用户输入]
                  │
          Hook: before_prompt_build          ← [AD-16, 来源: OpenClaw]
                  │
                  ▼
          Hook: before_model_resolve
                  │
                  ▼
         ┌── LLM 调用 ◄──────────────┐
         │                            │
         ▼                            │
    解析响应                           │
    /        \                        │
   ▼          ▼                       │
 文本输出   ToolCall(s)               │
   │          │                       │
   │   Hook: before_tool_call         │
   │          │                       │
   │          ▼                       │
   │   循环检测 (SHA256 去重)          │    ← [AD-13, 来源: OpenFang]
   │   ├── 检测到循环 → 熔断 [完成]    │
   │   └── 正常 ↓                     │
   │          │                       │
   │   SecurityGate::authorize        │
   │          │                       │
   │   凭证注入 (宿主边界)             │    ← [AD-10, 来源: IronClaw]
   │          │                       │
   │   执行工具 (T1/T2/T3)            │
   │          │                       │
   │   DLP 双向扫描                    │    ← [AD-11, 来源: IronClaw]
   │          │                       │
   │   Hook: after_tool_call          │
   │          │                       │
   ▼          ▼                       │
 [完成]  追加结果 ─────────────────────┘
              │
       迭代/Token 预算检查
       超限 → 强制终止 [完成]

 Memory Flush: 上下文压缩前触发静默回合持久化关键记忆    ← [来源: OpenClaw]
```

**循环检测算法**（来源: OpenFang）：
- 维护最近 N 次 tool-call 的 SHA256 指纹队列
- 指纹 = SHA256(tool_name + sorted(arguments))
- 连续出现 >= 3 次相同指纹 → 触发熔断，强制终止并返回错误提示

**8 个生命周期 Hook 点**（来源: OpenClaw）：

| Hook | 阶段 | 用途 |
|------|------|------|
| `before_prompt_build` | Prompt 构建前 | 注入额外上下文 |
| `before_model_resolve` | 模型选择前 | 动态切换 Provider/Model |
| `before_tool_call` | 工具执行前 | 拦截/修改/审批 |
| `after_tool_call` | 工具执行后 | 结果后处理 |
| `on_tool_error` | 工具出错时 | 自定义错误处理 |
| `before_response` | 最终回复前 | 回复过滤/修改 |
| `on_compaction` | 上下文压缩时 | 自定义压缩策略 |
| `on_session_end` | 会话结束时 | 清理/持久化 |

### 3.3 安全门控 (`clawx-security`)

**8 层纵深防御架构**（融合 IronClaw + OpenFang + ZeroClaw）：

```
Layer 1: Rust 语言安全         — 编译时消除内存漏洞
Layer 2: WASM 沙箱 + 双计量     — T1 工具隔离 (Fuel + Epoch)         [OpenFang]
Layer 3: 宿主边界凭证注入       — LLM/工具永远看不到密钥              [IronClaw]
Layer 4: 双向泄漏扫描 + 污点跟踪 — 出站/入站 DLP + Taint 标签传播     [IronClaw+OpenFang]
Layer 5: Merkle 审计链          — 每条日志 hash 链接，不可篡改        [OpenFang]
Layer 6: Prompt 注入防御        — 结构隔离 + 模式扫描 + LLM 自检     [IronClaw]
Layer 7: 循环检测 + 熔断        — SHA256 指纹去重，防止工具循环烧钱    [OpenFang]
Layer 8: 网络策略代理 + 白名单   — 所有出站经代理，域名白名单+IP封锁   [IronClaw+ZeroClaw]
```

```rust
pub enum ExecutionTier {
    /// T1: WASM 沙箱 + Fuel/Epoch 双计量
    Sandboxed,
    /// T2: 受限子进程 + 命令白名单 + 工作区隔离
    Subprocess,
    /// T3: 原生执行。完整用户权限
    Native,
}

/// seL4 式 Capability 模型 (来源: IronClaw AD-14)
/// 零访问默认，显式授予每一项能力
pub struct Capability {
    pub kind: CapabilityKind,
    pub granted_by: AgentId,       // 谁授予的
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub enum CapabilityKind {
    FileRead(PathBuf),             // 读取指定路径
    FileWrite(PathBuf),            // 写入指定路径
    HttpRequest(String),           // 指定域名的 HTTP 请求
    ShellCommand(String),          // 指定命令（如 "git", "cargo"）
    SecretInject(String),          // 注入指定名称的密钥
    DelegateToAgent(AgentId),      // 委派任务给指定 Agent
}

pub enum SecurityDecision {
    Allowed(Vec<Capability>),       // 授予的能力列表
    RequiresUserConfirmation(String),
    Denied(String),
}

#[async_trait]
pub trait SecurityGate: Send + Sync {
    /// Capability 检查：工具是否被授予所需的能力
    async fn authorize_tool(
        &self, agent_id: &AgentId, tool_name: &str,
        arguments: &serde_json::Value, tier: ExecutionTier,
    ) -> Result<SecurityDecision>;

    /// 双向 DLP 扫描 (来源: IronClaw)
    async fn dlp_scan(&self, content: &str, direction: DataDirection) -> Result<DlpResult>;

    /// Prompt 注入防御 (3 层: 模式匹配 + 内容净化 + LLM 自检)
    async fn prompt_injection_scan(&self, input: &str) -> Result<InjectionScanResult>;

    /// Merkle chain 审计记录
    async fn audit(&self, event: AuditEvent) -> Result<()>;

    /// 验证审计链完整性
    async fn verify_audit_chain(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<bool>;
}

/// 宿主边界凭证注入器 (来源: IronClaw AD-10)
#[async_trait]
pub trait CredentialInjector: Send + Sync {
    /// 将凭证注入到 HTTP 请求中。凭证从 Keychain 读取，绑定到域名策略。
    /// LLM 和工具代码全程不可见密钥原文。
    async fn inject_credentials(&self, request: &mut HttpRequest) -> Result<()>;

    /// 注册域名→凭证绑定策略
    async fn register_policy(&self, domain: &str, credential_name: &str) -> Result<()>;
}

/// 污点跟踪 (来源: OpenFang)
pub struct TaintLabel {
    pub source: String,             // 如 "api_key:openai", "user_pii:email"
    pub classification: TaintLevel, // 秘密级别
}

pub enum TaintLevel {
    Secret,     // API key, 密码
    Pii,        // 个人可识别信息
    Internal,   // 内部数据
    Public,     // 可公开
}
```

### 3.4 三层执行模型

| 层级 | 沙箱 | 文件系统 | 网络 | 用例 | 用户同意 |
|------|------|---------|------|------|---------|
| **T1: Sandboxed** | WASM (Wasmtime) + Fuel 计量 + Epoch 中断 | 无（或显式 WASI 路径） | 无 | 纯计算：文本转换、JSON 解析 | 无需（默认） |
| **T2: Subprocess** | OS 进程 + sandbox-exec + 命令白名单 | 限定工作目录 + 路径净化 | 仅代理（白名单） | Shell、Python、浏览器自动化 | 首次授权后记住 |
| **T3: Native** | 无 | 完整用户权限 | 完整 | 系统管理、sudo | 每次显式确认 |

**WASM 双重计量**（来源: OpenFang AD-12）：
- **Fuel 计量**：每个 WASM 指令消耗 fuel，到达上限后自动停止
- **Epoch 中断**：独立看门狗线程按固定间隔推进 epoch，超时触发中断
- 两者互为补充：fuel 防止 CPU 死循环，epoch 防止阻塞式等待

**工作区隔离**（来源: ZeroClaw）：
- Agent 文件操作限定在工作区目录内
- 阻断：路径遍历（`..`）、符号链接跟随、null byte 注入
- T2 子进程仅允许白名单命令（如 `git`, `cargo`, `python`）

### 3.4.1 宿主边界凭证注入（来源: IronClaw AD-10）

```
                    ┌─────────────────────────────┐
                    │     LLM Context Window       │
                    │  (永远不包含任何密钥/Token)    │
                    └──────────┬──────────────────┘
                               │ tool_call: http_request(url, body)
                               ▼
                    ┌─────────────────────────────┐
                    │     SecurityGate             │
                    │  authorize + DLP scan        │
                    └──────────┬──────────────────┘
                               │
                               ▼
                    ┌─────────────────────────────┐
                    │  CredentialInjector (宿主层)  │  ← 密钥仅在此层可见
                    │  1. 查找 domain→credential   │
                    │  2. 注入 Authorization header │
                    │  3. 出站 DLP 扫描             │
                    └──────────┬──────────────────┘
                               │ (密钥已注入的 HTTP 请求)
                               ▼
                    ┌─────────────────────────────┐
                    │     External Service         │
                    └──────────┬──────────────────┘
                               │ (响应)
                               ▼
                    ┌─────────────────────────────┐
                    │  入站 DLP 扫描                │  ← 检测凭证反射
                    │  (扫描响应中是否包含密钥)      │
                    └──────────┬──────────────────┘
                               │ (净化后的响应)
                               ▼
                    ┌─────────────────────────────┐
                    │     Tool Result → LLM        │
                    └─────────────────────────────┘
```

**关键原则**：凭证从 macOS Keychain 读取 → 绑定到策略规则（domain → credential）→ 仅在宿主层 HTTP 执行时注入。LLM、Prompt、工具代码全程看不到密钥原文。

### 3.4.2 Merkle 审计链（来源: OpenFang AD-11）

```
 AuditEntry #1          AuditEntry #2          AuditEntry #3
 ┌──────────────┐       ┌──────────────┐       ┌──────────────┐
 │ data: {...}   │       │ data: {...}   │       │ data: {...}   │
 │ prev_hash: ∅  │──────▶│ prev_hash: H1 │──────▶│ prev_hash: H2 │
 │ hash: H1      │       │ hash: H2      │       │ hash: H3      │
 └──────────────┘       └──────────────┘       └──────────────┘

 H1 = SHA256(data_1)
 H2 = SHA256(H1 + data_2)
 H3 = SHA256(H2 + data_3)

 篡改任何一条 → 后续所有 hash 断裂 → 立即可检测
```

替换原有的简单 HMAC 审计日志。数据库 schema 更新：
```sql
CREATE TABLE audit_log (
    id         TEXT PRIMARY KEY,
    timestamp  TEXT NOT NULL,
    agent_id   TEXT,
    module     TEXT NOT NULL,
    action     TEXT NOT NULL,
    severity   TEXT NOT NULL,
    detail     TEXT NOT NULL,      -- JSON
    prev_hash  TEXT NOT NULL,      -- 前一条的 hash（首条为空字符串）
    hash       TEXT NOT NULL       -- SHA256(prev_hash + detail)
);
```

### 3.5 记忆中心 (`clawx-memory`)

```rust
pub enum MemoryLayer { Working, Agent, User }

pub struct MemoryEntry {
    pub id: MemoryId,
    pub layer: MemoryLayer,
    pub kind: MemoryKind,         // Fact, Preference, Event, Skill
    pub summary: String,
    pub detail: serde_json::Value,
    pub importance: f32,          // 0.0 - 10.0
    pub freshness: f32,           // 随时间衰减
    pub pinned: bool,
    pub source_agent: Option<AgentId>,
    pub created_at: DateTime<Utc>,
    pub last_accessed: DateTime<Utc>,
    pub access_count: u64,
}

#[async_trait]
pub trait MemoryStore: Send + Sync {
    async fn store(&self, entry: MemoryEntry) -> Result<MemoryId>;
    async fn retrieve(&self, query: &MemoryQuery) -> Result<Vec<MemoryEntry>>;
    async fn update(&self, id: MemoryId, update: MemoryUpdate) -> Result<()>;
    async fn delete(&self, id: MemoryId) -> Result<()>;
    async fn decay_pass(&self) -> Result<u64>;

    /// Memory Flush (来源: OpenClaw)
    /// 上下文压缩前触发静默 LLM 回合，让模型主动持久化关键记忆
    async fn flush_before_compaction(
        &self, agent_id: &AgentId, context: &ConversationContext,
    ) -> Result<Vec<MemoryEntry>>;

    /// 智能记忆注入 (来源: OpenFang)
    /// 根据当前上下文智能选择相关记忆注入 Prompt，避免不必要的 memory_recall 循环
    async fn smart_inject(
        &self, agent_id: &AgentId, current_input: &str, limit: usize,
    ) -> Result<Vec<MemoryEntry>>;
}
```

### 3.6 知识库引擎 (`clawx-kb`)

```rust
pub struct SearchRequest {
    pub query: String,
    pub collection: Option<String>,
    pub top_k: usize,                // 默认: 5
    pub min_score: Option<f32>,
    pub filters: Vec<MetadataFilter>,
}

pub struct SearchResult {
    pub chunk_id: ChunkId,
    pub content: String,
    pub score: f32,                   // RRF 融合分数
    pub source_file: PathBuf,
    pub metadata: serde_json::Value,
}

/// 混合检索参数 (来源: OpenClaw)
pub struct RetrievalConfig {
    pub vector_weight: f32,           // 默认 0.7
    pub bm25_weight: f32,             // 默认 0.3
    pub mmr_lambda: f32,              // MMR 去重系数，默认 0.7 (来源: OpenClaw)
    pub temporal_decay_half_life_days: u32,  // 时间衰减半衰期，默认 30 天 (来源: OpenClaw)
}

#[async_trait]
pub trait KnowledgeEngine: Send + Sync {
    async fn add_source(&self, path: PathBuf, config: IndexConfig) -> Result<()>;
    async fn remove_source(&self, path: &Path) -> Result<()>;
    /// 混合检索: 向量 + BM25 + RRF 融合 + MMR 去重 + 时间衰减
    async fn search(&self, request: &SearchRequest) -> Result<Vec<SearchResult>>;
    async fn indexing_status(&self, path: &Path) -> Result<IndexingStatus>;
}
```

**检索降级策略**（来源: ZeroClaw）：
- 默认: Qdrant 向量搜索 + Tantivy BM25 + RRF 融合
- 降级: 无 Qdrant 时，使用 SQLite FTS5 + sqlite-vec 实现混合检索（嵌入式方案）

### 3.7 事件总线 (`clawx-eventbus`)

```rust
pub enum EventKind {
    // 系统
    SystemStartup, SystemShutdown,
    DiskSpaceLow { available_bytes: u64 },
    MemoryPressure { used_mb: u64 },
    // Agent
    AgentCreated { agent_id: AgentId },
    AgentStarted { agent_id: AgentId },
    TaskCompleted { agent_id: AgentId, task_id: TaskId },
    // 渠道
    MessageReceived { channel_id: ChannelId, agent_id: AgentId },
    ChannelConnected { channel_id: ChannelId },
    // 安全
    DlpBlocked { agent_id: AgentId, pattern: String },
    InjectionDetected { agent_id: AgentId },
    // Skills
    SkillInstalled { skill_id: SkillId },
    SkillExecutionComplete { skill_id: SkillId, duration_ms: u64 },
    // 知识库
    IndexingStarted { source: PathBuf },
    IndexingComplete { source: PathBuf, chunks: u64 },
    // 硬件
    DeviceConnected { device_type: String },
    PowerStateChanged { state: PowerState },
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: Event) -> Result<()>;
    fn subscribe(&self, filter: EventFilter) -> EventReceiver;
}
```

**实现方案**：`tokio::broadcast`（buffer 4096）作为主总线 + 每订阅者独立 `mpsc` 通道（带过滤和背压）。关键事件由专门的"事件持久化"订阅者写入审计日志。

### 3.8 渠道适配器 (`clawx-channel`)

```rust
/// 归一化消息 (来源: ZeroClaw ChannelMessage 模式)
/// 所有平台消息归一化为此统一类型
pub struct IncomingMessage {
    pub channel_id: ChannelId,
    pub sender: ChannelUser,
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub is_direct: bool,           // DM vs 群组
    pub thread_id: Option<String>,
    pub guild_id: Option<String>,  // 团队/组织 ID (用于 Binding Rules)
    pub account_id: Option<String>,// 渠道账号 ID
    pub raw: serde_json::Value,    // 原始平台载荷
}

#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    async fn connect(&mut self) -> Result<()>;
    async fn disconnect(&mut self) -> Result<()>;
    async fn send_message(&self, message: OutgoingMessage) -> Result<()>;
    fn receive_stream(&self) -> Pin<Box<dyn Stream<Item = IncomingMessage> + Send>>;
    fn status(&self) -> ChannelStatus;
    fn platform(&self) -> ChannelPlatform;
}
```

### 3.9 网关 (`clawx-gateway`)（来源: OpenClaw AD-16）

```rust
/// Binding Rules: 4 级确定性路由 (来源: OpenClaw)
/// 优先级: Peer > Guild > Account > Channel
pub struct BindingRule {
    pub agent_id: AgentId,
    pub priority: BindingPriority,
    pub matcher: BindingMatcher,
}

pub enum BindingPriority {
    Peer,      // 精确匹配 DM/群组 ID（最高优先级）
    Guild,     // 匹配团队/组织 ID
    Account,   // 匹配渠道账号 ID
    Channel,   // 渠道全局兜底（最低优先级）
}

pub enum BindingMatcher {
    ExactPeer(String),    // 精确 peer ID
    GuildId(String),
    AccountId(String),
    Platform(ChannelPlatform),
}

/// Lane Queue: 每会话序列化执行 (来源: OpenClaw)
/// 防止同一会话的并发请求竞争状态
pub struct SessionLane {
    pub session_key: String,       // agent_id + channel_id + peer_id
    pub queue: tokio::sync::mpsc::Sender<LaneTask>,
}

#[async_trait]
pub trait Gateway: Send + Sync {
    /// 根据 Binding Rules 将消息路由到目标 Agent
    fn resolve_agent(&self, message: &IncomingMessage) -> Option<AgentId>;

    /// 将消息放入对应 Session Lane 队列
    async fn enqueue(&self, agent_id: AgentId, message: IncomingMessage) -> Result<()>;

    /// 认证请求（API 调用时）
    async fn authenticate(&self, token: &str) -> Result<AuthContext>;

    /// 限流检查
    async fn rate_limit_check(&self, key: &str) -> Result<bool>;
}
```

**Lane 队列工作模式**：
```
Channel A (Telegram) ──┐
Channel B (飞书)     ──┤──▶ Gateway ──▶ Binding Rules ──▶ Agent X
Channel C (API)      ──┘       │                              │
                               ▼                              ▼
                         Session Lane                    Agent Loop
                     (session_key 序列化)              (一次处理一条)
```

### 3.10 Hook 系统 (`clawx-runtime`)（来源: OpenClaw）

```rust
/// 8 个生命周期 Hook 点
#[async_trait]
pub trait AgentHook: Send + Sync {
    /// Prompt 构建前：注入额外上下文
    async fn before_prompt_build(&self, ctx: &mut PromptContext) -> Result<()> { Ok(()) }

    /// 模型选择前：动态切换 Provider/Model
    async fn before_model_resolve(&self, ctx: &mut ModelContext) -> Result<()> { Ok(()) }

    /// 工具执行前：拦截/修改/审批
    async fn before_tool_call(&self, call: &mut ToolCall) -> Result<HookDecision> {
        Ok(HookDecision::Continue)
    }

    /// 工具执行后：结果后处理
    async fn after_tool_call(&self, call: &ToolCall, result: &mut String) -> Result<()> { Ok(()) }

    /// 工具出错时：自定义错误处理
    async fn on_tool_error(&self, call: &ToolCall, error: &ClawxError) -> Result<ErrorAction> {
        Ok(ErrorAction::Propagate)
    }

    /// 最终回复前：回复过滤/修改
    async fn before_response(&self, response: &mut String) -> Result<()> { Ok(()) }

    /// 上下文压缩时：触发 Memory Flush
    async fn on_compaction(&self, ctx: &mut CompactionContext) -> Result<()> { Ok(()) }

    /// 会话结束时：清理/持久化
    async fn on_session_end(&self, session: &SessionInfo) -> Result<()> { Ok(()) }
}

pub enum HookDecision { Continue, Skip, Abort(String) }
pub enum ErrorAction { Propagate, Retry, Ignore, Custom(String) }
```

### 3.11 Observer 可观测 (`clawx-types`)（来源: IronClaw）

```rust
/// 可插拔系统观测点
#[async_trait]
pub trait Observer: Send + Sync {
    /// Agent Loop 每次迭代时通知
    async fn on_iteration(&self, agent_id: &AgentId, iteration: u32, tokens_used: u64) {}

    /// 工具执行时通知（含耗时）
    async fn on_tool_execution(&self, agent_id: &AgentId, tool: &str, duration: Duration) {}

    /// LLM 调用时通知（含用量）
    async fn on_llm_call(&self, agent_id: &AgentId, provider: &str, usage: &TokenUsage) {}

    /// 安全事件通知
    async fn on_security_event(&self, event: &AuditEvent) {}

    /// 系统指标上报
    async fn on_metrics(&self, metrics: &SystemMetrics) {}
}

pub struct SystemMetrics {
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub disk_available_bytes: u64,
    pub active_agents: u32,
    pub active_sessions: u32,
    pub pending_tasks: u32,
}
```

### 3.9 硬件抽象 (`clawx-hal`)

```rust
#[async_trait]
pub trait AcceleratorProvider: Send + Sync {
    async fn infer(&self, model: &OnnxModel, input: &Tensor) -> Result<Tensor>;
    fn capabilities(&self) -> AcceleratorCapabilities;
}

#[async_trait]
pub trait StorageProvider: Send + Sync {
    async fn create_snapshot(&self, label: &str) -> Result<SnapshotId>;
    async fn list_snapshots(&self) -> Result<Vec<SnapshotInfo>>;
    async fn restore_snapshot(&self, id: &SnapshotId, target: &Path) -> Result<()>;
    async fn delete_snapshot(&self, id: &SnapshotId) -> Result<()>;
}
```

---

## 4. 数据流

### 4.1 GUI 用户消息流

```
SwiftUI View → ViewModel → UniFFI bridge → clawx-ffi
  → clawx-gateway::authenticate()
  → clawx-runtime::dispatch(agent_id, message)
  → Agent Loop:
       → clawx-security::prompt_injection_scan(input)
       → clawx-memory::retrieve(相关记忆)
       → clawx-kb::search(相关知识)
       → 构建 Prompt (system + memories + knowledge + user input)
       → clawx-llm::complete(prompt)
       → 解析响应:
            ToolCall → security::authorize → skills::execute → security::dlp_scan → 循环
            TextOutput → security::dlp_scan → memory::store(摘要) → 返回
  → UniFFI bridge (callback/stream) → SwiftUI 更新
```

### 4.2 IM 渠道消息流

```
IM 平台 (如 Telegram)
  → clawx-channel::TelegramAdapter.receive_stream()
  → clawx-gateway::route(channel_id → agent_id)
  → clawx-runtime::dispatch(agent_id, message)
  → [同 4.1 的 Agent Loop]
  → clawx-channel::send_message(response)
```

### 4.3 主动式 Agent 流

```
clawx-scheduler::cron_trigger(task_config)
  → clawx-eventbus::publish(ScheduledTaskFired)
  → clawx-runtime::dispatch(agent_id, task_prompt)
  → [Agent Loop]
  → clawx-channel::send_message(result)  // IM 推送
  AND/OR → macOS notification (via clawx-ffi)
```

---

## 5. 数据库 Schema

所有数据库使用 SQLite WAL 模式。按领域分离数据库文件以减少写竞争。

### 5.1 核心数据库 (`~/.clawx/data/core.db`)

```sql
CREATE TABLE agents (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    role          TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    model_provider TEXT NOT NULL,
    model_name    TEXT NOT NULL,
    model_params  TEXT,              -- JSON
    status        TEXT NOT NULL DEFAULT 'idle',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE agent_skills (
    agent_id  TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    skill_id  TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    enabled   INTEGER NOT NULL DEFAULT 1,
    config    TEXT,
    PRIMARY KEY (agent_id, skill_id)
);

CREATE TABLE agent_channels (
    agent_id    TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    channel_id  TEXT NOT NULL,
    platform    TEXT NOT NULL,
    config      TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'disconnected',
    PRIMARY KEY (agent_id, channel_id)
);

CREATE TABLE tasks (
    id             TEXT PRIMARY KEY,
    agent_id       TEXT NOT NULL REFERENCES agents(id),
    parent_task_id TEXT,
    delegated_to   TEXT,
    type           TEXT NOT NULL,    -- user_request|scheduled|delegated|event_driven
    input          TEXT NOT NULL,
    status         TEXT NOT NULL DEFAULT 'pending',
    result         TEXT,
    error          TEXT,
    created_at     TEXT NOT NULL,
    started_at     TEXT,
    completed_at   TEXT
);

CREATE TABLE scheduled_jobs (
    id              TEXT PRIMARY KEY,
    agent_id        TEXT NOT NULL REFERENCES agents(id),
    name            TEXT NOT NULL,
    cron_expr       TEXT NOT NULL,
    task_prompt     TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    last_run        TEXT,
    next_run        TEXT,
    notify_channels TEXT              -- JSON array
);

CREATE TABLE skills (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    version        TEXT NOT NULL,
    description    TEXT,
    manifest       TEXT NOT NULL,     -- JSON
    wasm_path      TEXT,
    execution_tier TEXT NOT NULL DEFAULT 'sandboxed',
    signature      TEXT,
    installed_at   TEXT NOT NULL,
    source         TEXT               -- store|local|opensource
);
```

### 5.2 记忆数据库 (`~/.clawx/data/memory.db`)

```sql
CREATE TABLE agent_memories (
    id            TEXT PRIMARY KEY,
    agent_id      TEXT NOT NULL,
    kind          TEXT NOT NULL,
    summary       TEXT NOT NULL,
    detail        TEXT,
    importance    REAL NOT NULL DEFAULT 5.0,
    freshness     REAL NOT NULL DEFAULT 10.0,
    pinned        INTEGER NOT NULL DEFAULT 0,
    access_count  INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    last_accessed TEXT NOT NULL,
    embedding_id  TEXT
);

CREATE TABLE user_memories (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,
    summary       TEXT NOT NULL,
    detail        TEXT,
    importance    REAL NOT NULL DEFAULT 5.0,
    freshness     REAL NOT NULL DEFAULT 10.0,
    pinned        INTEGER NOT NULL DEFAULT 0,
    access_count  INTEGER NOT NULL DEFAULT 0,
    source_agent  TEXT,
    confirmed     INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    last_accessed TEXT NOT NULL,
    embedding_id  TEXT
);

CREATE TABLE conversations (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL,
    channel_id  TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    summary     TEXT
);

CREATE TABLE messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    tool_call_id    TEXT,
    token_count     INTEGER,
    created_at      TEXT NOT NULL
);
```

### 5.3 知识库数据库 (`~/.clawx/data/knowledge.db`)

```sql
CREATE TABLE kb_sources (
    id          TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',
    file_count  INTEGER NOT NULL DEFAULT 0,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    last_indexed TEXT,
    config      TEXT
);

CREATE TABLE kb_files (
    id          TEXT PRIMARY KEY,
    source_id   TEXT NOT NULL REFERENCES kb_sources(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    hash        TEXT NOT NULL,
    format      TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'pending',
    indexed_at  TEXT,
    error       TEXT
);

CREATE TABLE kb_chunks (
    id           TEXT PRIMARY KEY,
    file_id      TEXT NOT NULL REFERENCES kb_files(id) ON DELETE CASCADE,
    content      TEXT NOT NULL,
    chunk_index  INTEGER NOT NULL,
    token_count  INTEGER NOT NULL,
    embedding_id TEXT,
    metadata     TEXT
);
```

### 5.4 审计数据库 (`~/.clawx/data/audit.db`)

```sql
CREATE TABLE audit_log (
    id         TEXT PRIMARY KEY,
    timestamp  TEXT NOT NULL,
    agent_id   TEXT,
    module     TEXT NOT NULL,
    action     TEXT NOT NULL,
    severity   TEXT NOT NULL,     -- info|warning|critical
    detail     TEXT NOT NULL,     -- JSON
    prev_hash  TEXT NOT NULL,     -- Merkle chain: 前一条的 hash
    hash       TEXT NOT NULL      -- SHA256(prev_hash + detail)
);

CREATE TABLE usage_stats (
    id             TEXT PRIMARY KEY,
    agent_id       TEXT NOT NULL,
    provider       TEXT NOT NULL,
    model          TEXT NOT NULL,
    input_tokens   INTEGER NOT NULL,
    output_tokens  INTEGER NOT NULL,
    estimated_cost REAL,
    timestamp      TEXT NOT NULL
);
```

---

## 6. 跨切面关注点

### 6.1 错误处理

统一错误类型定义在 `clawx-types`：

```rust
#[derive(Debug, thiserror::Error)]
pub enum ClawxError {
    #[error("LLM provider error: {provider}: {message}")]
    LlmProvider { provider: String, message: String },
    #[error("LLM rate limited, retry after {retry_after_secs}s")]
    LlmRateLimited { retry_after_secs: u64 },
    #[error("Tool execution denied: {reason}")]
    SecurityDenied { reason: String },
    #[error("DLP violation: {pattern} in {direction}")]
    DlpViolation { pattern: String, direction: String },
    #[error("Prompt injection detected")]
    PromptInjection { details: String },
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("WASM execution error: {0}")]
    WasmExecution(String),
    #[error("WASM timeout after {timeout_secs}s")]
    WasmTimeout { timeout_secs: u64 },
    #[error("Channel {platform} error: {message}")]
    ChannelConnection { platform: String, message: String },
    #[error("Config error: {0}")]
    Config(String),
    #[error("Internal: {0}")]
    Internal(#[from] anyhow::Error),
}
```

### 6.2 日志

所有 crate 使用 `tracing` + 结构化字段。安全规则：INFO 及以上**永不**记录 API Key、密码、PII、完整 Prompt。

### 6.3 配置

```
~/.clawx/
  config/
    clawx.toml               # 主配置
    network-whitelist.toml    # 网络白名单
    skills/                   # 每 Skill 配置覆盖
  data/
    core.db, memory.db, knowledge.db, audit.db
  qdrant/                     # Qdrant 嵌入式数据
  tantivy/                    # Tantivy 索引
  artifacts/                  # Agent 生成的文件
  wasm/                       # 已安装 WASM Skill 二进制
  logs/                       # tracing 输出
```

### 6.4 数据库迁移

使用 `sqlx::migrate!()`，迁移文件命名：`YYYYMMDDHHMMSS_description.sql`。守护进程启动时自动执行迁移。永不在迁移中删除列，仅标记废弃。

---

## 7. 修订版路线图

基于 PRD 问题分析，调整优先级和排期：

| 阶段 | 版本 | 周期 | 交付物 | 优先级 |
|------|------|------|--------|--------|
| **奠基** | v0.1 | 14 周 | types, config, eventbus, llm, runtime (Agent Loop), memory (基础), vault, security (基础 DLP + 路径限制), ffi, SwiftUI 单 Agent GUI | P0 |
| **生态** | v0.2 | 12 周 | security (完整 WASM 沙箱), skills, channel (飞书+TG), gateway, scheduler, 多 Agent, kb (混合检索) | P0 |
| **加固** | v0.3 | 10 周 | daemon (看门狗), artifact, 账号体系, api, 用量统计, 远程 SSH, Samba/WebDAV | P1 |
| **硬件** | v0.4 | 8 周 | hal, ota, 录音豆, VM 支持 | P1 |
| **商业** | v0.5 | 8 周 | Skills 商店, 云端备份, Agent 分享, OpenClaw 迁移 | P1 |
| **社区** | v1.0 | 10 周 | HireClaw, 移动端 App, Agent 商业化 | P2 |

**关键变更**（v2.1 竞品分析后追加）：
- v0.1 新增：宿主边界凭证注入、Merkle 审计链、Agent Loop 循环检测
- v0.2 新增：WASM 双重计量、Hands 自治包、Gateway Lane 队列、8 个 Hook 点
- v0.3 新增：污点跟踪、WIT 工具接口

---

## 8. Hands 自治 Agent 包（来源: OpenFang AD-15）

Hands 是 ClawX 实现"主动式 Agent"的核心模式。每个 Hand 是一个自包含的自治 Agent 包，可以 24/7 独立运行。

### 8.1 Hand 结构

```
hands/
  researcher/
    HAND.toml          # 清单：名称、调度规则、权限、通知渠道
    SYSTEM.md          # 系统提示 + 多阶段操作手册
    SKILL.md           # 专家知识文档
    settings.toml      # 可配置参数
```

**HAND.toml 示例：**
```toml
[hand]
name = "AI-Safety-Researcher"
description = "每日追踪 AI Safety 相关论文和动态"
agent_id = "researcher-bot"

[schedule]
cron = "0 30 8 * * *"   # 每天 8:30
timezone = "Asia/Shanghai"

[permissions]
network_domains = ["arxiv.org", "scholar.google.com"]
fs_write = ["~/.clawx/artifacts/research/"]

[notifications]
channels = ["feishu-group-123", "desktop"]
on_complete = true
on_error = true

[budget]
max_iterations = 30
max_tokens = 200000
max_cost_usd = 0.50
```

### 8.2 Hand vs 普通定时任务

| 维度 | 普通定时任务 | Hand |
|------|------------|------|
| 配置 | 单行 Cron + Prompt | 完整清单 + 操作手册 + 知识 |
| 能力 | 发一条消息 | 多阶段执行、构建知识图谱、生成报告 |
| 监控 | 日志 | 仪表盘指标 + 运行历史 |
| 预算 | 无 | Token/费用/迭代上限 |
| 分发 | 不可分发 | 可打包分享到社区 |

---

## 9. 实现前必须决定的问题

| # | 问题 | 推荐 | 截止 |
|---|------|------|------|
| 1 | UniFFI vs swift-bridge | UniFFI（更广泛类型支持） | v0.1 开始前 |
| 2 | Agent Loop 迭代上限 | 可配置，默认 20 次迭代 / 100K token | v0.1 开始前 |
| 3 | 工作记忆实现 | 内存 HashMap，会话结束时刷入 SQLite | v0.1 开始前 |
| 4 | MCP 工具映射 | 专用 MCP 桥接进程，MCP Server 作为 T2 子进程 | v0.2 开始前 |
| 5 | 事件总线溢出策略 | 每订阅者独立 mpsc + 关键事件始终持久化 | v0.1 开始前 |
| 6 | SQLite 连接池 | deadpool 4 连接/数据库，WAL 模式 | v0.1 开始前 |
| 7 | Embedding 模型加载 | 懒加载，有 KB 数据源时预热 | v0.2 开始前 |
| 8 | 凭证注入实现方案 | macOS Keychain + domain→credential 策略表 | v0.1 开始前 |
| 9 | Merkle 审计链初始 hash | 空字符串作为创世记录的 prev_hash | v0.1 开始前 |
| 10 | WIT 接口版本 | WASI Preview 2 (wasm32-wasip2) | v0.2 开始前 |
