# ClawX 系统架构文档 v3.0

**版本:** 3.0
**日期:** 2026年3月17日
**基于:** PRD v2.0 + 架构 v2.1 + OpenClaw/IronClaw/ZeroClaw/OpenFang 竞品深度分析

> 竞品分析见 `docs/arch/competitive-analysis.md`
> 前版架构见 `docs/arch/architecture-v2.1.md`

### v2.1 → v3.0 变更摘要

| 变更类型 | 内容 |
|---------|------|
| **新增 7 个 crate** | clawx-account, clawx-sync, clawx-community, clawx-migration, clawx-physical, clawx-mobile-relay, clawx-settings |
| **重写 clawx-vault** | 从 APFS 快照改为工作区级内容寻址版本化 |
| **扩展 5 个 crate** | clawx-llm (模型注册), clawx-daemon (看门狗), clawx-ota (Delta 更新), clawx-skills (商店), clawx-scheduler (NL-to-Cron) |
| **新增 14 个 Trait** | VaultEngine, ArtifactStore, ModelRegistry, UsageAggregator, AuthProvider, SyncEngine, CommunityClient, MigrationEngine, DeviceAdapter, CameraAdapter, SmartHomeAdapter, RelayServer, SettingsStore, Watchdog |
| **新增 2 个数据库** | vault.db, account.db |
| **新增 8 条数据流** | Vault、Account、Backup、Physical、Mobile、OTA、Migration、Skills Store |
| **新增 7 个 AD** | AD-17 ~ AD-23 |
| **新增 PRD 追溯矩阵** | 全部 19 个 PRD 模块 → crate/trait/table/flow 映射 |

---

## 1. 架构概览

ClawX 是本地优先的 Agent Computer 平台，采用 **Rust 核心 + SwiftUI 原生 GUI**。本文档定义模块边界、接口、数据流和跨切面关注点。

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
| **AD-17**: 工作区级版本化 (非 APFS) | PRD 要求以 ClawX Workspace 为边界，APFS 快照粒度过大、权限要求高，改为内容寻址 blob store + changeset 记录 | PRD v2.0 §2.2 |
| **AD-18**: `clawx-account` 可选依赖 | 本地闭环层永远不依赖账号体系；account crate 仅被 sync/community/skills-store 引用 | PRD v2.0 §1.4 |
| **AD-19**: 移动端 Relay 采用 gRPC + WireGuard | gRPC streaming 天然支持流式输出；WireGuard 解决 NAT 穿透，端到端加密 | PRD v2.0 §2.18 |
| **AD-20**: 物理设备协议适配器模式 | RTSP/ONVIF/MQTT/HomeKit 各自实现 `DeviceAdapter` trait，统一事件模型 | PRD v2.0 §2.17 |
| **AD-21**: NL-to-Cron 采用 LLM function-call | 自然语言解析为 Cron 表达式的最可靠方式，regex 无法覆盖中文表述 | PRD v2.0 §2.8 |
| **AD-22**: Delta OTA 采用 bsdiff + Ed25519 | bsdiff 补丁体积小；Ed25519 签名验证快、密钥短 | PRD v2.0 §2.16 |
| **AD-23**: i18n 采用 fluent-rs | Mozilla Fluent 格式支持复杂本地化规则（复数、性别），编译时键验证 | PRD v2.0 §2.19 |

---

## 2. Rust Workspace Crate 结构

```
clawx/
  Cargo.toml                    (workspace root)
  crates/
    clawx-types/                共享类型、错误分类、事件定义、Observer trait
    clawx-config/               配置加载、验证、Schema
    clawx-eventbus/             发布-订阅事件总线
    clawx-llm/                  LLM Provider 抽象 + 模型注册 + 用量聚合
    clawx-runtime/              Agent Loop + Hook 系统 + 循环检测 + 任务分发
    clawx-memory/               两层记忆 + Memory Flush + 智能注入
    clawx-kb/                   知识库引擎 (embedding + 混合检索 + MMR + 时间衰减)
    clawx-security/             8 层纵深防御:
                                  - Capability 模型 (seL4 式)
                                  - 凭证注入器 (宿主边界)
                                  - DLP 双向扫描 + 污点跟踪
                                  - Merkle 审计链
                                  - Prompt 注入防御
                                  - 网络策略代理
                                  - 工作区隔离 + 路径净化
    clawx-vault/                工作区版本化：内容寻址 blob store + changeset + 文件级还原
    clawx-skills/               Skill WASM 加载 + Fuel/Epoch 双计量 + WIT 接口 + 商店客户端
    clawx-scheduler/            Cron + 事件驱动 + Hands 自治包引擎 + NL-to-Cron
    clawx-channel/              IM 渠道适配器 + 消息归一化 (ZeroClaw 模式)
    clawx-gateway/              Binding Rules 4 级路由 + Lane 队列 + 认证限流
    clawx-artifact/             文件产物管理 + 预览生成 + 导出分享
    clawx-hal/                  硬件抽象 (ONNX Runtime EP)
    clawx-daemon/               看门狗 + 进程守护 + 心跳检测 + 内存泄漏监控 + launchd 集成
    clawx-ota/                  OTA 更新 (Delta bsdiff + A/B 分区 + Ed25519 签名)
    clawx-account/              OAuth 2.0/OIDC 客户端 + JWT 验证 + 会话管理
    clawx-sync/                 增量同步引擎 + AES-256-GCM 加密备份 + WebDAV/iCloud 适配
    clawx-community/            HireClaw 社区 API 客户端 + Agent 发布/搜索/下载
    clawx-migration/            OpenClaw 数据检测 + 格式转换 + 增量迁移
    clawx-physical/             设备发现 (mDNS) + RTSP 摄像头 + MQTT/HomeKit 桥接
    clawx-mobile-relay/         gRPC streaming relay + WireGuard 隧道协调
    clawx-settings/             i18n (fluent-rs) + 主题管理 + 用户偏好持久化
    clawx-api/                  RESTful API (axum)
    clawx-ffi/                  UniFFI 桥接到 Swift
  hands/                        Hands 自治 Agent 包 (TOML + SYSTEM.md + SKILL.md)
  locales/
    en/                         英文 Fluent 翻译包
    zh-CN/                      简体中文 Fluent 翻译包
  apps/
    clawx-gui/                  SwiftUI macOS 应用
    clawx-cli/                  CLI 工具
    clawx-service/              后台守护进程 binary
    clawx-mobile/               iOS SwiftUI 应用 (远程 Relay 客户端)
```

### 2.1 依赖关系图

```
clawx-types          (叶子节点：无内部依赖。含 Observer trait, 共享 ID 类型)
clawx-config         -> types
clawx-eventbus       -> types
clawx-settings       -> types, config
clawx-llm            -> types, config
clawx-security       -> types, config, eventbus
clawx-memory         -> types, llm, eventbus
clawx-kb             -> types, llm, eventbus
clawx-vault          -> types, eventbus
clawx-skills         -> types, security, eventbus
clawx-scheduler      -> types, eventbus, skills, llm (NL-to-Cron)
clawx-channel        -> types, eventbus
clawx-gateway        -> types, channel, security
clawx-artifact       -> types, eventbus
clawx-physical       -> types, eventbus, security
clawx-runtime        -> types, llm, memory, kb, security,
                        skills, eventbus, scheduler, vault, gateway, artifact
clawx-hal            -> types
clawx-daemon         -> types, eventbus, runtime
clawx-ota            -> types, hal, security (签名验证)
clawx-account        -> types, config
clawx-sync           -> types, config, account, security (加密)
clawx-community      -> types, account, skills
clawx-migration      -> types, config, memory, kb
clawx-mobile-relay   -> types, api, account, security
clawx-api            -> runtime, gateway
clawx-ffi            -> runtime, api, settings
```

**依赖原则**：
- `clawx-types` 是唯一的叶子节点，所有 crate 直接或间接依赖它
- 本地闭环层 crate（runtime, memory, kb, vault, security）不依赖 account/sync/community
- 平台生态层 crate（account, sync, community）作为可选边缘依赖，不反向侵入核心

### 2.2 核心 Trait 总览

融合 ZeroClaw 5 Trait + IronClaw 10 Trait 精华，扩展至覆盖全部 PRD 模块：

| Trait | Crate | 来源 | 职责 |
|-------|-------|------|------|
| `LlmProvider` | clawx-llm | ZeroClaw | LLM 调用抽象 |
| `ModelRegistry` | clawx-llm | **v3.0 新增** | 模型注册、Per-Agent 绑定、Key 轮换 |
| `UsageAggregator` | clawx-llm | **v3.0 新增** | 用量统计与费用估算 |
| `AgentExecutor` | clawx-runtime | 原创 | Agent Loop 执行 |
| `AgentHook` | clawx-runtime | OpenClaw | 8 个生命周期扩展点 |
| `SecurityGate` | clawx-security | IronClaw | 授权 + DLP + 审计 |
| `CredentialInjector` | clawx-security | IronClaw | 宿主边界凭证注入 |
| `MemoryStore` | clawx-memory | ZeroClaw | 两层记忆 + Flush + 智能注入 |
| `KnowledgeEngine` | clawx-kb | 原创 | 混合检索 + MMR + 时间衰减 |
| `VaultEngine` | clawx-vault | **v3.0 重写** | 工作区版本化 + changeset + 文件级还原 |
| `ArtifactStore` | clawx-artifact | **v3.0 新增** | 文件产物管理 + 预览 + 导出 |
| `ChannelAdapter` | clawx-channel | ZeroClaw | 渠道适配 + 消息归一化 |
| `Gateway` | clawx-gateway | OpenClaw | Binding Rules + Lane 队列 |
| `EventBus` | clawx-eventbus | 原创 | 发布-订阅解耦 |
| `Observer` | clawx-types | IronClaw | 可插拔系统观测 |
| `AcceleratorProvider` | clawx-hal | 原创 | AI 推理硬件抽象 |
| `Watchdog` | clawx-daemon | **v3.0 新增** | 进程守护 + 心跳 + 内存监控 |
| `UpdateEngine` | clawx-ota | **v3.0 新增** | Delta OTA + 签名验证 + 回滚 |
| `AuthProvider` | clawx-account | **v3.0 新增** | OAuth/OIDC 登录 + 会话管理 |
| `SyncEngine` | clawx-sync | **v3.0 新增** | 加密备份 + 增量同步 |
| `CommunityClient` | clawx-community | **v3.0 新增** | Agent 社区发布/搜索/下载 |
| `MigrationEngine` | clawx-migration | **v3.0 新增** | OpenClaw 数据迁移 |
| `DeviceAdapter` | clawx-physical | **v3.0 新增** | 物理设备协议适配 |
| `RelayServer` | clawx-mobile-relay | **v3.0 新增** | 移动端 gRPC relay |
| `SettingsStore` | clawx-settings | **v3.0 新增** | 偏好持久化 + i18n + 主题 |
| `SkillsStoreClient` | clawx-skills | **v3.0 新增** | Skills 商店浏览/购买/许可 |

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

### 3.2 模型注册与绑定 (`clawx-llm`) — v3.0 新增

```rust
/// 模型 Provider 配置（存储在 core.db 的 model_providers 表）
pub struct ProviderConfig {
    pub id: ProviderId,
    pub name: String,
    pub provider_type: ProviderType,        // OpenAI, Anthropic, Ollama, Custom
    pub base_url: Option<String>,
    pub keychain_ref: String,               // macOS Keychain 引用，非明文
    pub rotation_policy: Option<KeyRotationPolicy>,
    pub default_model: String,
    pub default_params: ModelParams,
}

pub struct ModelParams {
    pub temperature: f32,
    pub max_tokens: u32,
    pub top_p: Option<f32>,
    pub stop_sequences: Vec<String>,
}

pub struct KeyRotationPolicy {
    pub rotate_after_days: u32,             // 自动轮换天数
    pub max_usage_count: Option<u64>,       // 达到使用次数后轮换
}

/// Per-Agent 模型绑定
pub struct AgentModelBinding {
    pub agent_id: AgentId,
    pub provider_id: ProviderId,
    pub model_name: String,                 // 如 "claude-opus-4-6", "gpt-4o"
    pub params_override: Option<ModelParams>,// Agent 级参数覆盖
}

#[async_trait]
pub trait ModelRegistry: Send + Sync {
    /// 注册新的 Provider 配置
    async fn register_provider(&self, config: ProviderConfig) -> Result<ProviderId>;

    /// 更新 Provider 配置
    async fn update_provider(&self, id: &ProviderId, config: ProviderConfig) -> Result<()>;

    /// 删除 Provider
    async fn remove_provider(&self, id: &ProviderId) -> Result<()>;

    /// 列出所有已注册 Provider
    async fn list_providers(&self) -> Result<Vec<ProviderConfig>>;

    /// 绑定 Agent 到指定模型
    async fn bind_agent_model(&self, binding: AgentModelBinding) -> Result<()>;

    /// 获取 Agent 绑定的模型（用于 Agent Loop 内部调用）
    async fn resolve_agent_model(&self, agent_id: &AgentId) -> Result<(Box<dyn LlmProvider>, String)>;

    /// 测试 Provider 连通性
    async fn test_connection(&self, provider_id: &ProviderId) -> Result<ConnectionTestResult>;

    /// 轮换 API Key（从 Keychain 更新）
    async fn rotate_key(&self, provider_id: &ProviderId, new_keychain_ref: &str) -> Result<()>;
}

/// 用量统计聚合
pub struct UsageRecord {
    pub agent_id: AgentId,
    pub provider_id: ProviderId,
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub estimated_cost_usd: f64,
    pub timestamp: DateTime<Utc>,
}

pub struct UsageQuery {
    pub agent_id: Option<AgentId>,
    pub provider_id: Option<ProviderId>,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub granularity: UsageGranularity,      // Daily, Weekly, Monthly
}

pub enum UsageGranularity { Daily, Weekly, Monthly }

pub struct UsageSummary {
    pub total_input_tokens: u64,
    pub total_output_tokens: u64,
    pub total_cost_usd: f64,
    pub request_count: u64,
    pub breakdown: Vec<UsageBucket>,        // 按粒度分桶
}

#[async_trait]
pub trait UsageAggregator: Send + Sync {
    /// 记录一次 LLM 调用用量（Agent Loop 每次调用后自动记录）
    async fn record(&self, record: UsageRecord) -> Result<()>;

    /// 按维度查询用量统计
    async fn query(&self, query: &UsageQuery) -> Result<UsageSummary>;

    /// 获取单个 Agent 的总费用
    async fn agent_total_cost(&self, agent_id: &AgentId) -> Result<f64>;
}
```

### 3.3 Agent Loop (`clawx-runtime`)

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
   │   VaultEngine::create_version    │    ← [AD-17, 文件操作前自动版本点]
   │          │                       │
   │   执行工具 (T1/T2/T3)            │
   │          │                       │
   │   DLP 双向扫描                    │    ← [AD-11, 来源: IronClaw]
   │          │                       │
   │   ArtifactStore::register        │    ← [文件产物自动登记]
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

### 3.4 安全门控 (`clawx-security`)

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
    pub granted_by: AgentId,
    pub granted_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

pub enum CapabilityKind {
    FileRead(PathBuf),
    FileWrite(PathBuf),
    HttpRequest(String),
    ShellCommand(String),
    SecretInject(String),
    DelegateToAgent(AgentId),
    DeviceControl(DeviceId),              // v3.0 新增：物理设备控制
}

pub enum SecurityDecision {
    Allowed(Vec<Capability>),
    RequiresUserConfirmation(String),
    Denied(String),
}

#[async_trait]
pub trait SecurityGate: Send + Sync {
    /// Capability 检查
    async fn authorize_tool(
        &self, agent_id: &AgentId, tool_name: &str,
        arguments: &serde_json::Value, tier: ExecutionTier,
    ) -> Result<SecurityDecision>;

    /// 双向 DLP 扫描
    async fn dlp_scan(&self, content: &str, direction: DataDirection) -> Result<DlpResult>;

    /// Prompt 注入防御 (3 层)
    async fn prompt_injection_scan(&self, input: &str) -> Result<InjectionScanResult>;

    /// Merkle chain 审计记录
    async fn audit(&self, event: AuditEvent) -> Result<()>;

    /// 验证审计链完整性
    async fn verify_audit_chain(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<bool>;
}

/// 宿主边界凭证注入器 (来源: IronClaw AD-10)
#[async_trait]
pub trait CredentialInjector: Send + Sync {
    async fn inject_credentials(&self, request: &mut HttpRequest) -> Result<()>;
    async fn register_policy(&self, domain: &str, credential_name: &str) -> Result<()>;
}

/// 污点跟踪 (来源: OpenFang)
pub struct TaintLabel {
    pub source: String,
    pub classification: TaintLevel,
}

pub enum TaintLevel { Secret, Pii, Internal, Public }
```

### 3.5 三层执行模型

| 层级 | 沙箱 | 文件系统 | 网络 | 用例 | 用户同意 |
|------|------|---------|------|------|---------|
| **T1: Sandboxed** | WASM (Wasmtime) + Fuel 计量 + Epoch 中断 | 无（或显式 WASI 路径） | 无 | 纯计算：文本转换、JSON 解析 | 无需（默认） |
| **T2: Subprocess** | OS 进程 + sandbox-exec + 命令白名单 | 限定工作目录 + 路径净化 | 仅代理（白名单） | Shell、Python、浏览器自动化 | 首次授权后记住 |
| **T3: Native** | 无 | 完整用户权限 | 完整 | 系统管理、sudo、设备控制 | 每次显式确认 |

**WASM 双重计量**（来源: OpenFang AD-12）：
- **Fuel 计量**：每个 WASM 指令消耗 fuel，到达上限后自动停止
- **Epoch 中断**：独立看门狗线程按固定间隔推进 epoch，超时触发中断
- 两者互为补充：fuel 防止 CPU 死循环，epoch 防止阻塞式等待

**工作区隔离**（来源: ZeroClaw）：
- Agent 文件操作限定在工作区目录内
- 阻断：路径遍历（`..`）、符号链接跟随、null byte 注入
- T2 子进程仅允许白名单命令（如 `git`, `cargo`, `python`）

### 3.5.1 宿主边界凭证注入（来源: IronClaw AD-10）

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

### 3.5.2 Merkle 审计链（来源: OpenFang AD-11）

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

### 3.6 记忆中心 (`clawx-memory`)

```rust
pub enum MemoryLayer { Agent, User }

pub struct MemoryEntry {
    pub id: MemoryId,
    pub layer: MemoryLayer,
    pub kind: MemoryKind,         // Fact, Preference, Event, Skill
    pub summary: String,
    pub detail: serde_json::Value,
    pub importance: f32,          // 0.0 - 10.0
    pub freshness: f32,           // 随时间衰减（艾宾浩斯遗忘曲线）
    pub pinned: bool,             // 永久保留标记
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

    /// 艾宾浩斯衰减：遍历所有记忆，按时间衰减 freshness，低于阈值归档/删除
    async fn decay_pass(&self) -> Result<u64>;

    /// Memory Flush (来源: OpenClaw)
    /// 上下文压缩前触发静默 LLM 回合，让模型主动持久化关键记忆
    async fn flush_before_compaction(
        &self, agent_id: &AgentId, context: &ConversationContext,
    ) -> Result<Vec<MemoryEntry>>;

    /// 智能记忆注入 (来源: OpenFang)
    /// 根据当前上下文智能选择相关记忆注入 Prompt
    async fn smart_inject(
        &self, agent_id: &AgentId, current_input: &str, limit: usize,
    ) -> Result<Vec<MemoryEntry>>;
}
```

**两层记忆架构**（对应 PRD §2.3）：

| 层级 | 作用域 | 内容 | 权限 | 存储 |
|------|--------|------|------|------|
| **Agent 记忆** | 单个 Agent | 历史对话摘要、学到的技能、任务日志 | 仅所属 Agent 读写 | memory.db: agent_memories |
| **用户记忆** | 全局共享 | 姓名、职业、偏好、联系人、术语 | 所有 Agent 可读写 | memory.db: user_memories |

### 3.7 知识库引擎 (`clawx-kb`)

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

/// 混合检索参数
pub struct RetrievalConfig {
    pub vector_weight: f32,           // 默认 0.7
    pub bm25_weight: f32,             // 默认 0.3
    pub mmr_lambda: f32,              // MMR 去重系数，默认 0.7
    pub temporal_decay_half_life_days: u32,  // 时间衰减半衰期，默认 30 天
}

#[async_trait]
pub trait KnowledgeEngine: Send + Sync {
    /// 添加知识源文件夹（触发 FSEvents 监控 + 增量索引）
    async fn add_source(&self, path: PathBuf, config: IndexConfig) -> Result<()>;
    async fn remove_source(&self, path: &Path) -> Result<()>;
    /// 混合检索: 向量 + BM25 + RRF 融合 + MMR 去重 + 时间衰减
    async fn search(&self, request: &SearchRequest) -> Result<Vec<SearchResult>>;
    async fn indexing_status(&self, path: &Path) -> Result<IndexingStatus>;
}
```

**多格式解析管线**（对应 PRD §2.4.3）：

| 格式 | 解析策略 | Rust crate |
|------|---------|-----------|
| `.txt`, `.md`, `.csv`, `.json` | 直接文本读取 | 标准库 |
| `.pdf` | 文本 + 图片提取 | `pdf-extract` |
| `.docx`, `.pptx`, `.xlsx` | Office 文档提取 | `docx-rs` / Python 桥接 |
| `.jpg`, `.png`, `.webp` | CLIP 多模态 Embedding | ONNX via clawx-hal |
| `.mp3`, `.wav` | Whisper 转录后 Embedding | ONNX via clawx-hal |
| `.mp4`, `.mov` | 关键帧提取 + Whisper 音轨转录 | ffmpeg CLI (T2) + ONNX |

**智能分块策略**：基于语义分块，保持段落和句子完整性。Chunk 目标 512 Token，相邻重叠 10%。

**本地 Embedding 模型**：
- 文本：`nomic-embed-text`（轻量）或 `bge-m3`（多语言）
- 多模态：`CLIP ViT-B/32`（文本+图像统一空间）
- 要求：模型体积 < 500MB、推理延迟 < 50ms/chunk、支持 Apple Silicon (CoreML/Metal)

**检索降级策略**（来源: ZeroClaw）：
- 默认: Qdrant 向量搜索 + Tantivy BM25 + RRF 融合
- 降级: 无 Qdrant 时，使用 SQLite FTS5 + sqlite-vec 实现混合检索

### 3.8 事件总线 (`clawx-eventbus`)

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
    // 硬件 / 物理设备
    DeviceConnected { device_id: DeviceId, device_type: String },
    DeviceDisconnected { device_id: DeviceId },
    DeviceEvent { device_id: DeviceId, event: DeviceEventPayload },
    PowerStateChanged { state: PowerState },
    // Vault
    VersionPointCreated { version_id: VersionId, agent_id: AgentId },
    RestoreCompleted { version_id: VersionId },
    // OTA
    UpdateAvailable { version: String, channel: String },
    UpdateApplied { from: String, to: String },
    // 主动任务反馈
    ProactiveFeedbackReceived { task_id: TaskId, feedback: FeedbackType },
}

#[async_trait]
pub trait EventBus: Send + Sync {
    async fn publish(&self, event: Event) -> Result<()>;
    fn subscribe(&self, filter: EventFilter) -> EventReceiver;
}
```

**实现方案**：`tokio::broadcast`（buffer 4096）作为主总线 + 每订阅者独立 `mpsc` 通道（带过滤和背压）。关键事件由专门的"事件持久化"订阅者写入审计日志。

### 3.9 渠道适配器 (`clawx-channel`)

```rust
/// 归一化消息 (来源: ZeroClaw ChannelMessage 模式)
pub struct IncomingMessage {
    pub channel_id: ChannelId,
    pub sender: ChannelUser,
    pub content: String,
    pub attachments: Vec<Attachment>,
    pub is_direct: bool,
    pub thread_id: Option<String>,
    pub guild_id: Option<String>,
    pub account_id: Option<String>,
    pub raw: serde_json::Value,
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

**支持的渠道**（对应 PRD §2.6.3）：

| 渠道 | 接入方式 | ChannelPlatform 枚举 |
|------|---------|---------------------|
| 飞书/Lark | WebSocket 长连接 | `Feishu` |
| Telegram | Bot API Long-polling | `Telegram` |
| Slack | WebSocket (Socket Mode) | `Slack` |
| WhatsApp | Web API | `WhatsApp` |
| Discord | WebSocket (Gateway) | `Discord` |
| 微信企业版 | Webhook 回调 | `WeCom` |

### 3.10 网关 (`clawx-gateway`)（来源: OpenClaw AD-16）

```rust
/// Binding Rules: 4 级确定性路由
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
    ExactPeer(String),
    GuildId(String),
    AccountId(String),
    Platform(ChannelPlatform),
}

/// Lane Queue: 每会话序列化执行
pub struct SessionLane {
    pub session_key: String,       // agent_id + channel_id + peer_id
    pub queue: tokio::sync::mpsc::Sender<LaneTask>,
}

#[async_trait]
pub trait Gateway: Send + Sync {
    fn resolve_agent(&self, message: &IncomingMessage) -> Option<AgentId>;
    async fn enqueue(&self, agent_id: AgentId, message: IncomingMessage) -> Result<()>;
    async fn authenticate(&self, token: &str) -> Result<AuthContext>;
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

### 3.11 Hook 系统 (`clawx-runtime`)（来源: OpenClaw）

```rust
#[async_trait]
pub trait AgentHook: Send + Sync {
    async fn before_prompt_build(&self, ctx: &mut PromptContext) -> Result<()> { Ok(()) }
    async fn before_model_resolve(&self, ctx: &mut ModelContext) -> Result<()> { Ok(()) }
    async fn before_tool_call(&self, call: &mut ToolCall) -> Result<HookDecision> {
        Ok(HookDecision::Continue)
    }
    async fn after_tool_call(&self, call: &ToolCall, result: &mut String) -> Result<()> { Ok(()) }
    async fn on_tool_error(&self, call: &ToolCall, error: &ClawxError) -> Result<ErrorAction> {
        Ok(ErrorAction::Propagate)
    }
    async fn before_response(&self, response: &mut String) -> Result<()> { Ok(()) }
    async fn on_compaction(&self, ctx: &mut CompactionContext) -> Result<()> { Ok(()) }
    async fn on_session_end(&self, session: &SessionInfo) -> Result<()> { Ok(()) }
}

pub enum HookDecision { Continue, Skip, Abort(String) }
pub enum ErrorAction { Propagate, Retry, Ignore, Custom(String) }
```

### 3.12 Observer 可观测 (`clawx-types`)（来源: IronClaw）

```rust
#[async_trait]
pub trait Observer: Send + Sync {
    async fn on_iteration(&self, agent_id: &AgentId, iteration: u32, tokens_used: u64) {}
    async fn on_tool_execution(&self, agent_id: &AgentId, tool: &str, duration: Duration) {}
    async fn on_llm_call(&self, agent_id: &AgentId, provider: &str, usage: &TokenUsage) {}
    async fn on_security_event(&self, event: &AuditEvent) {}
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

### 3.13 硬件抽象 (`clawx-hal`)

```rust
#[async_trait]
pub trait AcceleratorProvider: Send + Sync {
    async fn infer(&self, model: &OnnxModel, input: &Tensor) -> Result<Tensor>;
    fn capabilities(&self) -> AcceleratorCapabilities;
}
```

### 3.14 数据保险箱 (`clawx-vault`) — v3.0 重写

**设计原则**（AD-17）：
- 以 ClawX Workspace (`~/.clawx/workspace/`) 为版本化边界，不使用 APFS 系统快照
- 内容寻址 blob store：文件内容以 SHA256 为键存储，天然去重
- 每次 Agent 文件操作前自动创建版本点，记录 changeset
- 支持文件级、任务级还原

```rust
/// 版本点
pub struct VersionPoint {
    pub id: VersionId,
    pub agent_id: Option<AgentId>,
    pub task_id: Option<TaskId>,
    pub label: String,                     // clawx-{agent_id}-{task_id}-{timestamp}
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// 变更集
pub struct ChangeSet {
    pub version_id: VersionId,
    pub changes: Vec<FileChange>,
}

pub struct FileChange {
    pub file_path: PathBuf,                // 工作区内相对路径
    pub change_type: ChangeType,
    pub before_blob_hash: Option<String>,  // SHA256, None 表示新增
    pub after_blob_hash: Option<String>,   // SHA256, None 表示删除
    pub before_size: Option<u64>,
    pub after_size: Option<u64>,
}

pub enum ChangeType { Added, Modified, Deleted, Renamed { from: PathBuf } }

/// 差异预览
pub struct DiffPreview {
    pub version: VersionPoint,
    pub changes: Vec<FileChange>,
    pub total_added: u64,
    pub total_modified: u64,
    pub total_deleted: u64,
}

/// 清理策略
pub struct RetentionPolicy {
    pub full_retention_days: u32,          // 7 天内全保留
    pub daily_retention_days: u32,         // 7-30 天每天保留 1 个
    pub max_retention_days: u32,           // 30 天以上自动删除
    pub disk_warning_threshold_percent: u8, // 磁盘剩余 < 此百分比时警告
}

#[async_trait]
pub trait VaultEngine: Send + Sync {
    /// Agent 文件操作前自动创建版本点
    async fn create_version_point(
        &self, agent_id: &AgentId, task_id: Option<&TaskId>, description: Option<&str>,
    ) -> Result<VersionId>;

    /// 记录文件变更到版本点
    async fn record_change(
        &self, version_id: &VersionId, change: FileChange,
    ) -> Result<()>;

    /// 存储文件内容到 blob store（自动去重）
    async fn store_blob(&self, content: &[u8]) -> Result<String>; // 返回 SHA256 hash

    /// 读取 blob 内容
    async fn read_blob(&self, hash: &str) -> Result<Vec<u8>>;

    /// 列出所有版本点（支持分页和筛选）
    async fn list_versions(
        &self, filter: Option<VersionFilter>,
    ) -> Result<Vec<VersionPoint>>;

    /// 差异预览：回滚前查看变更
    async fn diff_version(&self, version_id: &VersionId) -> Result<DiffPreview>;

    /// 文件级还原：恢复单个文件或文件夹
    async fn restore_file(
        &self, version_id: &VersionId, file_path: &Path,
    ) -> Result<()>;

    /// 任务级回滚：以任务为单位回滚所有变更
    async fn restore_task(&self, task_id: &TaskId) -> Result<()>;

    /// Skills 安装版本点（配置变更前自动创建）
    async fn create_skill_install_point(
        &self, skill_id: &SkillId, description: &str,
    ) -> Result<VersionId>;

    /// 执行智能清理（按 RetentionPolicy）
    async fn cleanup(&self, policy: &RetentionPolicy) -> Result<CleanupReport>;
}
```

**Blob Store 存储结构**：
```
~/.clawx/vault/
  blobs/
    ab/cd/abcdef1234...     # SHA256 前 2 字节作为分级目录
    12/34/1234abcdef...
  metadata.db               # vault.db (版本点 + changeset 元数据)
```

**文件进入工作区规则**（对应 PRD §2.2.4）：
- 对话拖拽文件 → 复制到 `~/.clawx/workspace/imports/` → 受版本保护
- 知识库源文件夹 → 只读索引，不纳入工作区回滚
- 工作区外写回 → 默认禁止，需用户显式确认导出
- 外部文件 → 仅工作区内副本受版本点保护

### 3.15 文件产物管理 (`clawx-artifact`) — v3.0 新增

```rust
pub struct Artifact {
    pub id: ArtifactId,
    pub agent_id: AgentId,
    pub task_id: Option<TaskId>,
    pub filename: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub path: PathBuf,                     // ~/.clawx/artifacts/ 下的绝对路径
    pub preview_path: Option<PathBuf>,     // 缩略图/首页预览
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

pub struct ArtifactQuery {
    pub agent_id: Option<AgentId>,
    pub mime_type_prefix: Option<String>,  // 如 "application/pdf", "image/"
    pub tags: Vec<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub sort_by: ArtifactSort,             // CreatedAt, Size, Name
    pub limit: usize,
    pub offset: usize,
}

pub struct CleanupPolicy {
    pub max_total_size_bytes: u64,         // 最大总存储
    pub max_age_days: u32,                 // 过期天数
}

#[async_trait]
pub trait ArtifactStore: Send + Sync {
    /// 注册新产物（Agent Loop 在工具生成文件后自动调用）
    async fn register(&self, artifact: Artifact) -> Result<ArtifactId>;

    /// 查询产物列表（支持筛选、排序、分页）
    async fn list(&self, query: &ArtifactQuery) -> Result<Vec<Artifact>>;

    /// 获取单个产物详情
    async fn get(&self, id: &ArtifactId) -> Result<Artifact>;

    /// 删除产物（同时删除文件和预览）
    async fn delete(&self, id: &ArtifactId) -> Result<()>;

    /// 导出到指定目录
    async fn export(&self, id: &ArtifactId, target_dir: &Path) -> Result<PathBuf>;

    /// 通过渠道分享（生成消息内容，交给 ChannelAdapter 发送）
    async fn share_via_channel(
        &self, id: &ArtifactId, channel_id: &ChannelId,
    ) -> Result<OutgoingMessage>;

    /// 生成预览（图片缩略图、PDF 首页、代码语法高亮）
    async fn generate_preview(&self, id: &ArtifactId) -> Result<Option<PathBuf>>;

    /// 按策略清理过期产物
    async fn cleanup(&self, policy: &CleanupPolicy) -> Result<u64>;
}
```

### 3.16 Skills 生态系统 (`clawx-skills`) 扩展 — v3.0 新增

在已有的 WASM 加载 + WIT 接口 + Fuel/Epoch 双计量基础上，新增商店和商业化支持：

```rust
/// Skill 本地管理（已有）
pub struct SkillManifest {
    pub id: SkillId,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: String,
    pub execution_tier: ExecutionTier,
    pub permissions: Vec<CapabilityKind>,   // 所需权限
    pub wasm_path: Option<PathBuf>,
    pub signature: Option<String>,          // Ed25519 签名
    pub source: SkillSource,
}

pub enum SkillSource { Local, OpenSource(String), Store(String) }

/// Skills 商店客户端 (v3.0 新增)
pub struct SkillListing {
    pub id: SkillId,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub rating: f32,
    pub download_count: u64,
    pub pricing: SkillPricing,
    pub tags: Vec<String>,
    pub screenshots: Vec<String>,
    pub verified: bool,                    // 通过四层安全检测
}

pub enum SkillPricing {
    Free,
    OneTimePurchase { price_usd: f64 },
    Subscription { monthly_usd: f64 },
    PayPerUse { per_use_usd: f64 },
}

pub struct SkillLicense {
    pub skill_id: SkillId,
    pub license_type: SkillPricing,
    pub purchased_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub receipt: String,                   // 加密购买凭证
}

pub struct StoreSearchQuery {
    pub keyword: Option<String>,
    pub tags: Vec<String>,
    pub sort_by: StoreSort,                // Rating, Downloads, Recent
    pub pricing_filter: Option<SkillPricing>,
    pub limit: usize,
    pub offset: usize,
}

#[async_trait]
pub trait SkillsStoreClient: Send + Sync {
    /// 浏览商店（分类、热门、新上架）
    async fn browse(&self, query: &StoreSearchQuery) -> Result<Vec<SkillListing>>;

    /// 搜索 Skills
    async fn search(&self, query: &StoreSearchQuery) -> Result<Vec<SkillListing>>;

    /// 下载并安装 Skill（含签名验证）
    async fn download_and_install(&self, skill_id: &SkillId) -> Result<SkillManifest>;

    /// 购买 Skill（需登录账号）
    async fn purchase(&self, skill_id: &SkillId) -> Result<SkillLicense>;

    /// 检查许可证有效性
    async fn check_license(&self, skill_id: &SkillId) -> Result<bool>;

    /// 评价 Skill
    async fn rate(&self, skill_id: &SkillId, rating: u8, review: Option<&str>) -> Result<()>;
}
```

**Skills 安全卫士（四层防护）**（对应 PRD §2.7.2）：
1. **静态代码审计**：WASM 字节码扫描已知恶意模式（`clawx-security` 调用）
2. **动态沙箱测试**：在 T1 沙箱中试运行，监控 Fuel 消耗和异常行为
3. **权限最小化**：Capability 模型确保 Skill 只获得声明的权限
4. **行为审计日志**：所有 Skill 操作写入 Merkle 审计链

### 3.17 主动式 Agent (`clawx-scheduler`) 扩展 — v3.0 新增

在已有 Cron + Hands 引擎基础上，新增 NL-to-Cron、反馈循环和通知路由：

```rust
/// 触发类型（对应 PRD §2.8.3）
pub enum TriggerType {
    /// Cron 表达式定时触发
    Cron {
        expression: String,
        timezone: String,
    },
    /// 系统事件触发
    EventDriven {
        event_filter: EventFilter,
    },
    /// 上下文感知触发
    ContextAware {
        condition: String,                 // LLM 评估的条件描述
        check_interval: Duration,
    },
    /// 策略匹配触发
    PolicyMatch {
        keyword_patterns: Vec<String>,
        monitoring_sources: Vec<String>,
    },
}

/// 主动任务
pub struct ProactiveTask {
    pub id: TaskId,
    pub agent_id: AgentId,
    pub name: String,
    pub trigger: TriggerType,
    pub task_prompt: String,               // Agent 执行的 Prompt
    pub notify_channels: Vec<ChannelId>,   // 结果推送渠道
    pub notify_desktop: bool,              // macOS 桌面通知
    pub enabled: bool,
    pub last_run: Option<DateTime<Utc>>,
    pub next_run: Option<DateTime<Utc>>,
    pub status: ProactiveTaskStatus,
}

pub enum ProactiveTaskStatus {
    Active,
    Paused,
    Error(String),
}

/// 用户反馈（对应 PRD §2.8.4）
pub enum ProactiveFeedback {
    Adopted,                               // 用户采纳了结果
    Dismissed,                             // 用户忽略
    ReduceFrequency,                       // "降低频率"
    NeverAgain,                            // "不再提醒"
}

pub struct FeedbackStats {
    pub task_id: TaskId,
    pub total_runs: u64,
    pub adopted_count: u64,
    pub dismissed_count: u64,
    pub reduce_count: u64,
    pub never_count: u64,
    pub adoption_rate: f32,                // 采纳率
    pub negative_rate: f32,                // 负反馈率
}

/// NL-to-Cron：自然语言转 Cron 表达式
pub struct NlCronResult {
    pub cron_expression: String,
    pub timezone: String,
    pub human_readable: String,            // "每天早上 8:30"
    pub confidence: f32,
}

#[async_trait]
pub trait ProactiveScheduler: Send + Sync {
    /// 自然语言创建定时任务（对应 PRD §2.8.3 "对话创建定时任务"）
    /// 内部调用 LLM function-call 解析为 Cron 表达式
    async fn create_from_natural_language(
        &self, agent_id: &AgentId, nl_input: &str, task_prompt: &str,
    ) -> Result<ProactiveTask>;

    /// 直接创建任务（Cron/事件/策略 触发）
    async fn create_task(&self, task: ProactiveTask) -> Result<TaskId>;

    /// 列出所有任务
    async fn list_tasks(&self, agent_id: Option<&AgentId>) -> Result<Vec<ProactiveTask>>;

    /// 暂停 / 恢复 / 删除任务
    async fn pause_task(&self, task_id: &TaskId) -> Result<()>;
    async fn resume_task(&self, task_id: &TaskId) -> Result<()>;
    async fn delete_task(&self, task_id: &TaskId) -> Result<()>;

    /// 记录用户反馈
    async fn record_feedback(
        &self, task_id: &TaskId, feedback: ProactiveFeedback,
    ) -> Result<()>;

    /// 获取反馈统计（用于调整触发策略）
    async fn feedback_stats(&self, task_id: &TaskId) -> Result<FeedbackStats>;

    /// 多渠道通知路由：任务结果 → 确定渠道 → 扇出发送
    async fn notify_result(
        &self, task: &ProactiveTask, result: &str,
    ) -> Result<()>;
}
```

**NL-to-Cron 流程**（AD-21）：
```
用户: "帮我每天早上 8 点检查邮件并总结"
  → LLM function-call: parse_schedule("每天早上 8 点")
  → 返回: { cron: "0 0 8 * * *", tz: "Asia/Shanghai", readable: "每天 08:00" }
  → 用户确认 → 创建 ProactiveTask
```

### 3.18 账号体系 (`clawx-account`) — v3.0 新增

```rust
pub enum OAuthFlow {
    Google,
    Apple,
    EmailPassword { email: String, password: String },
    PhoneSms { phone: String, code: String },
}

pub struct Session {
    pub user_id: String,
    pub provider: OAuthFlow,
    pub access_token_ref: String,          // Keychain 引用
    pub refresh_token_ref: Option<String>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

pub struct UserProfile {
    pub user_id: String,
    pub display_name: String,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
}

#[async_trait]
pub trait AuthProvider: Send + Sync {
    /// 发起 OAuth 登录（打开系统浏览器）
    async fn login(&self, flow: OAuthFlow) -> Result<Session>;

    /// 注销（清除本地 Session 和 Keychain 中的 Token）
    async fn logout(&self) -> Result<()>;

    /// 刷新 Token
    async fn refresh_token(&self) -> Result<Session>;

    /// 获取当前会话（None 表示未登录）
    async fn get_session(&self) -> Result<Option<Session>>;

    /// 获取用户资料
    async fn get_profile(&self) -> Result<Option<UserProfile>>;
}

/// 账号门控中间件
/// 本地功能调用时直接通过；云端功能调用时检查登录状态
pub struct AccountGate {
    auth: Arc<dyn AuthProvider>,
}

impl AccountGate {
    /// 需要登录的操作调用此方法
    pub async fn require_session(&self) -> Result<Session> {
        self.auth.get_session().await?.ok_or(ClawxError::AccountRequired {
            feature: "cloud feature".into(),
        })
    }
}
```

**设计原则**（AD-18）：
- 纯本地功能（对话、记忆、知识库、Vault）永远不调用 `AccountGate`
- 仅 sync、community、skills-store 等云端功能通过 `AccountGate` 检查登录

### 3.19 数据同步与云端备份 (`clawx-sync`) — v3.0 新增

```rust
pub enum SyncTarget {
    ICloudDrive { container: String },
    GoogleDrive { folder_id: String },
    WebDAV { url: String, username: String, keychain_ref: String },
}

/// 备份清单
pub struct BackupManifest {
    pub id: BackupId,
    pub version: String,                   // ClawX 版本号
    pub target: SyncTarget,
    pub content_list: Vec<BackupContent>,
    pub total_size_bytes: u64,
    pub encryption_key_ref: String,        // Keychain 引用（用户控制的密钥）
    pub checksum: String,                  // SHA256 of encrypted payload
    pub created_at: DateTime<Utc>,
}

pub enum BackupContent {
    AgentConfigs,
    UserMemories,
    AgentMemories,
    SkillSettings,
    UserPreferences,
    ScheduledTasks,
    ConversationHistory,
}

/// 同步状态跟踪（增量同步）
pub struct SyncState {
    pub entity_type: String,
    pub entity_id: String,
    pub local_version: u64,
    pub remote_version: Option<u64>,
    pub last_synced: Option<DateTime<Utc>>,
}

#[async_trait]
pub trait SyncEngine: Send + Sync {
    /// 创建全量加密备份
    async fn create_backup(
        &self, target: &SyncTarget, contents: &[BackupContent],
    ) -> Result<BackupManifest>;

    /// 列出云端所有备份
    async fn list_backups(&self, target: &SyncTarget) -> Result<Vec<BackupManifest>>;

    /// 恢复备份（下载 → 验证 → 解密 → 应用）
    async fn restore_backup(&self, manifest: &BackupManifest) -> Result<RestoreReport>;

    /// 增量同步（仅传输变更部分）
    async fn incremental_sync(&self, target: &SyncTarget) -> Result<SyncReport>;

    /// 配置自动定期备份
    async fn configure_auto_backup(
        &self, target: &SyncTarget, interval: Duration,
    ) -> Result<()>;
}

pub struct RestoreReport {
    pub agents_restored: u64,
    pub memories_restored: u64,
    pub skills_restored: u64,
    pub warnings: Vec<String>,             // 如：某 Skill 版本不兼容
    pub kb_rebuild_required: bool,         // 知识库需重新本地构建
}
```

**加密流程**：
```
备份数据 → 压缩 (zstd) → AES-256-GCM 加密 (密钥: 用户控制, 存 Keychain)
  → 上传到 SyncTarget → 存储 BackupManifest
```

**换机流程**（对应 PRD §2.12.2）：
```
新设备安装 ClawX → 登录账号 (clawx-account) → 列出备份 (clawx-sync)
  → 选择备份 → 下载 → 验证 checksum → 解密 → 应用配置/记忆/Skills
  → 知识库需用户手动迁移原始文件或通过云盘同步后重新构建索引
```

### 3.20 HireClaw 社区 (`clawx-community`) — v3.0 新增

```rust
pub struct AgentPackage {
    pub agent_config: serde_json::Value,   // AgentConfig 序列化
    pub skills_manifest: Vec<SkillId>,     // 依赖的 Skills
    pub system_prompt: String,
    pub metadata: AgentPackageMetadata,
    pub signature: String,                 // Ed25519 签名
}

pub struct AgentPackageMetadata {
    pub name: String,
    pub description: String,
    pub author: String,
    pub version: String,
    pub tags: Vec<String>,
    pub screenshots: Vec<String>,
    pub pricing: AgentPricing,
}

pub enum AgentPricing {
    Free,
    OneTimePurchase { price_usd: f64 },
    Subscription { monthly_usd: f64 },
    PayPerUse { per_use_usd: f64 },
}

pub struct AgentListing {
    pub id: String,
    pub unique_agent_id: String,           // 全局唯一 Agent 标识
    pub metadata: AgentPackageMetadata,
    pub rating: f32,
    pub download_count: u64,
    pub created_at: DateTime<Utc>,
}

pub struct AgentReview {
    pub reviewer_id: String,
    pub rating: u8,                        // 1-5
    pub comment: String,
    pub created_at: DateTime<Utc>,
}

#[async_trait]
pub trait CommunityClient: Send + Sync {
    /// 发布 Agent 到社区（需登录）
    async fn publish_agent(&self, package: AgentPackage) -> Result<String>;

    /// 搜索社区 Agent
    async fn search_agents(
        &self, keyword: &str, tags: &[String], sort: CommunitySort, limit: usize,
    ) -> Result<Vec<AgentListing>>;

    /// 下载并导入 Agent
    async fn download_agent(&self, agent_id: &str) -> Result<AgentPackage>;

    /// 评价 Agent
    async fn rate_agent(&self, agent_id: &str, review: AgentReview) -> Result<()>;

    /// 注册全局唯一 Agent ID（用于跨用户互联）
    async fn register_agent_id(&self, agent_id: &AgentId) -> Result<String>;

    /// Agent 间对话（需双方授权）
    async fn inter_agent_message(
        &self, from_agent: &AgentId, to_unique_id: &str, message: &str,
    ) -> Result<String>;
}
```

### 3.21 OpenClaw 迁移 (`clawx-migration`) — v3.0 新增

```rust
pub enum MigrationSource {
    OpenClaw { path: PathBuf },
}

pub struct MigrationPlan {
    pub source: MigrationSource,
    pub detected_items: MigrationItems,
    pub warnings: Vec<String>,
    pub estimated_duration: Duration,
}

pub struct MigrationItems {
    pub agents: Vec<MigrationItem>,
    pub conversations: Vec<MigrationItem>,
    pub skills: Vec<MigrationItem>,
    pub knowledge_data: Vec<MigrationItem>,
    pub api_keys: Vec<MigrationItem>,
}

pub struct MigrationItem {
    pub source_id: String,
    pub source_name: String,
    pub can_migrate: bool,
    pub issues: Vec<String>,               // 兼容性问题
}

pub enum MigrationStatus {
    Planned,
    InProgress { progress_percent: u8 },
    Completed,
    RolledBack,
    Failed(String),
}

#[async_trait]
pub trait MigrationEngine: Send + Sync {
    /// 自动检测本地 OpenClaw 安装
    async fn detect(&self) -> Result<Option<MigrationSource>>;

    /// 生成迁移计划（展示将要导入的数据清单）
    async fn plan(&self, source: &MigrationSource) -> Result<MigrationPlan>;

    /// 执行迁移（迁移前自动创建 Vault 版本点作为回滚点）
    async fn execute(&self, plan: &MigrationPlan) -> Result<MigrationStatus>;

    /// 回滚迁移（通过 Vault 版本点恢复）
    async fn rollback(&self, plan: &MigrationPlan) -> Result<()>;

    /// 增量同步（后续增量迁移新数据）
    async fn incremental_sync(&self, source: &MigrationSource) -> Result<MigrationStatus>;
}
```

**数据映射表**：

| OpenClaw 实体 | ClawX 实体 | 转换逻辑 |
|--------------|-----------|---------|
| Agent Config (JSON) | agents 表 | 字段映射 + system_prompt 迁移 |
| Conversations (SQLite/Markdown) | conversations + messages 表 | 格式转换 |
| Skills (npm 包) | skills 表 | 重新下载 WASM 版本（如有）或标记为不兼容 |
| Knowledge Data (向量) | 重新索引 | 原始文件路径迁移，重新分块和 Embedding |
| API Keys (文件/环境变量) | macOS Keychain | 加密迁移到 Keychain |

### 3.22 物理 Agent 接入 (`clawx-physical`) — v3.0 新增

```rust
/// 通用设备适配器
pub struct DeviceInfo {
    pub id: DeviceId,
    pub name: String,
    pub device_type: DeviceType,
    pub protocol: DeviceProtocol,
    pub address: String,                   // IP:Port 或 mDNS 名称
    pub status: DeviceStatus,
    pub capabilities: Vec<DeviceCapability>,
}

pub enum DeviceType { Camera, Light, Switch, Sensor, Thermostat, Lock, Speaker }
pub enum DeviceProtocol { Rtsp, Onvif, Mqtt, HomeKit, MiHome }
pub enum DeviceStatus { Online, Offline, Error(String) }
pub enum DeviceCapability { Stream, Capture, Control, Sense, Detect }

/// 设备事件
pub enum DeviceEventPayload {
    MotionDetected { zone: String, confidence: f32 },
    PersonDetected { zone: String, confidence: f32, snapshot_path: Option<PathBuf> },
    SensorReading { sensor_type: String, value: f64, unit: String },
    StateChanged { property: String, old_value: String, new_value: String },
    Alert { level: AlertLevel, message: String },
}

pub enum AlertLevel { Info, Warning, Critical }

#[async_trait]
pub trait DeviceAdapter: Send + Sync {
    /// 局域网自动发现设备（mDNS/SSDP）
    async fn discover(&self) -> Result<Vec<DeviceInfo>>;

    /// 手动添加设备
    async fn add_device(&self, config: DeviceConfig) -> Result<DeviceId>;

    /// 连接设备
    async fn connect(&self, device_id: &DeviceId) -> Result<()>;

    /// 断开设备
    async fn disconnect(&self, device_id: &DeviceId) -> Result<()>;

    /// 发送控制命令（如 "开灯", "调温度到 24 度"）
    async fn send_command(
        &self, device_id: &DeviceId, command: DeviceCommand,
    ) -> Result<CommandResult>;

    /// 订阅设备事件流
    fn subscribe_events(
        &self, device_id: &DeviceId,
    ) -> Pin<Box<dyn Stream<Item = DeviceEventPayload> + Send>>;

    /// 获取所有已接入设备状态
    async fn list_devices(&self) -> Result<Vec<DeviceInfo>>;
}

/// 摄像头专用适配器
#[async_trait]
pub trait CameraAdapter: DeviceAdapter {
    /// 开始 RTSP 流（返回本地 HLS 或帧流地址）
    async fn start_stream(&self, device_id: &DeviceId) -> Result<StreamHandle>;

    /// 截取当前帧
    async fn capture_frame(&self, device_id: &DeviceId) -> Result<PathBuf>;

    /// 配置智能检测（移动侦测、人形识别）
    async fn configure_detection(
        &self, device_id: &DeviceId, config: DetectionConfig,
    ) -> Result<()>;
}

/// 智能家居适配器
#[async_trait]
pub trait SmartHomeAdapter: DeviceAdapter {
    /// 控制设备（灯光亮度、开关、温度等）
    async fn control(
        &self, device_id: &DeviceId, property: &str, value: serde_json::Value,
    ) -> Result<()>;

    /// 获取设备当前状态
    async fn get_state(&self, device_id: &DeviceId) -> Result<serde_json::Value>;

    /// 创建自动化规则（设备事件 → Agent 行动联动）
    async fn create_automation(
        &self, trigger: DeviceEventPayload, action: AutomationAction,
    ) -> Result<AutomationId>;
}

pub struct DeviceCommand {
    pub action: String,                    // "turn_on", "set_temperature", "capture"
    pub params: serde_json::Value,
}

pub struct AutomationAction {
    pub agent_id: AgentId,
    pub task_prompt: String,               // Agent 执行的 Prompt
    pub notify_channels: Vec<ChannelId>,
}
```

**对话式控制集成**（对应 PRD §2.17.3）：
- Agent 注册设备控制工具：`control_device(device_id, action, params)`
- 用户说"关掉卧室的灯" → LLM 调用 `control_device("bedroom-light", "turn_off", {})`
- SecurityGate 检查 `CapabilityKind::DeviceControl` → 首次需确认 → 记住后自动放行

**设备事件 → Agent 联动**：
```
CameraAdapter::PersonDetected → EventBus::publish(DeviceEvent)
  → clawx-scheduler 匹配策略 → 触发 Agent 任务
  → Agent Loop: capture_frame() → send_notification(channel)
```

### 3.23 移动端随行 (`clawx-mobile-relay`) — v3.0 新增

```rust
/// gRPC 消息类型
pub struct ChatRequest {
    pub agent_id: AgentId,
    pub message: String,
    pub attachments: Vec<Vec<u8>>,
}

pub struct ChatResponseChunk {
    pub content: String,
    pub is_final: bool,
    pub tool_calls: Vec<ToolCall>,
}

pub struct NotificationPush {
    pub agent_id: AgentId,
    pub title: String,
    pub body: String,
    pub task_id: Option<TaskId>,
    pub timestamp: DateTime<Utc>,
}

pub struct AgentHealthInfo {
    pub agent_id: AgentId,
    pub name: String,
    pub status: String,
    pub last_active: DateTime<Utc>,
}

/// 设备配对
pub struct DevicePairing {
    pub device_id: String,
    pub device_name: String,
    pub public_key: Vec<u8>,               // 用于 mTLS
    pub paired_at: DateTime<Utc>,
}

#[async_trait]
pub trait RelayServer: Send + Sync {
    /// 启动 Relay 服务器（监听 gRPC + 建立 WireGuard 隧道）
    async fn start(&self, config: RelayConfig) -> Result<()>;

    /// 停止 Relay
    async fn stop(&self) -> Result<()>;

    /// 生成配对 QR 码（含连接信息 + 临时密钥）
    async fn generate_pairing_qr(&self) -> Result<PairingQrData>;

    /// 完成设备配对（交换证书）
    async fn complete_pairing(&self, pairing_data: &PairingResponse) -> Result<DevicePairing>;

    /// 认证移动端客户端（mTLS 证书验证）
    async fn authenticate_client(&self, cert: &[u8]) -> Result<DevicePairing>;

    /// 流式对话（gRPC streaming）
    async fn stream_chat(
        &self, request: ChatRequest,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<ChatResponseChunk>> + Send>>>;

    /// 推送通知到已配对设备
    async fn push_notification(&self, notification: NotificationPush) -> Result<()>;

    /// 获取所有 Agent 健康状态
    async fn get_agents_health(&self) -> Result<Vec<AgentHealthInfo>>;
}

pub struct RelayConfig {
    pub grpc_port: u16,                    // 默认: 50051
    pub tunnel_type: TunnelType,
    pub max_paired_devices: u8,            // 默认: 5
}

pub enum TunnelType {
    WireGuard { config_path: PathBuf },
    Tailscale,
    DirectLan,                             // 仅局域网（不穿透 NAT）
}
```

**连接架构**：
```
iOS App ◄── gRPC over TLS ──► WireGuard/Tailscale 隧道
                                     │
                              clawx-mobile-relay (Mac 端)
                                     │
                              clawx-gateway → Agent Loop
```

**配对流程**：
```
Mac 端生成 QR 码 (含 WireGuard peer config + 临时密钥)
  → iOS 扫码 → 建立 WireGuard 隧道 → 交换 mTLS 证书
  → 配对完成 → 后续连接自动验证
```

### 3.24 看门狗子系统 (`clawx-daemon`) 扩展 — v3.0 新增

```rust
/// 模块健康状态
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
    Unresponsive,                          // 心跳超时
}

pub struct ModuleHealth {
    pub module_name: String,
    pub status: HealthStatus,
    pub last_heartbeat: DateTime<Utc>,
    pub uptime: Duration,
    pub memory_mb: u64,
    pub cpu_percent: f32,
}

/// 心跳协议
pub struct HeartbeatConfig {
    pub interval: Duration,                // 默认: 5 秒
    pub timeout: Duration,                 // 默认: 15 秒（3 次未收到视为不响应）
}

/// 内存泄漏检测
pub struct MemoryMonitorConfig {
    pub sample_interval: Duration,         // 默认: 60 秒
    pub window_duration: Duration,         // 默认: 1 小时
    pub growth_threshold_percent: f32,     // 默认: 10%（1 小时内增长超过此值触发重启）
}

/// 进程恢复策略
pub struct RecoveryPolicy {
    pub max_restart_count: u32,            // 最大连续重启次数
    pub restart_window: Duration,          // 重启计数窗口
    pub backoff_strategy: BackoffStrategy,
    pub collect_crash_report: bool,
}

pub enum BackoffStrategy {
    Fixed(Duration),                       // 固定间隔
    Exponential { base: Duration, max: Duration },
}

/// 系统健康面板数据（供 GUI 展示）
pub struct SystemHealthPanel {
    pub overall_status: HealthStatus,
    pub modules: Vec<ModuleHealth>,
    pub system_metrics: SystemMetrics,
    pub recent_events: Vec<HealthEvent>,
    pub uptime: Duration,
}

pub struct HealthEvent {
    pub timestamp: DateTime<Utc>,
    pub module: String,
    pub event_type: String,                // "restart", "memory_warning", "crash"
    pub description: String,
}

#[async_trait]
pub trait Watchdog: Send + Sync {
    /// 启动看门狗（注册所有模块健康检查）
    async fn start(&self) -> Result<()>;

    /// 注册模块心跳（每个模块启动时调用）
    async fn register_module(&self, module_name: &str) -> Result<HeartbeatSender>;

    /// 获取系统健康面板数据
    async fn health_panel(&self) -> Result<SystemHealthPanel>;

    /// 手动触发模块重启
    async fn restart_module(&self, module_name: &str) -> Result<()>;

    /// 获取崩溃报告
    async fn crash_reports(&self) -> Result<Vec<CrashReport>>;
}

/// 心跳发送器（每个模块持有一个）
pub struct HeartbeatSender {
    tx: tokio::sync::mpsc::Sender<HeartbeatPayload>,
}

impl HeartbeatSender {
    /// 模块定期调用此方法报告自身状态
    pub async fn beat(&self, status: HealthStatus) -> Result<()>;
}
```

**进程守护集成**（对应 PRD §2.15.2）：
- `clawx-service` 注册为 macOS Launch Agent (`~/Library/LaunchAgents/com.clawx.service.plist`)
- 异常退出后 launchd 自动重启（`KeepAlive = true`）
- 看门狗内部二次守护：监控各模块心跳，超时则重启该模块
- 开机自启动 + 断电恢复后自动启动 + 恢复上次任务状态

**launchd plist 关键配置**：
```xml
<key>KeepAlive</key>
<true/>
<key>ThrottleInterval</key>
<integer>5</integer>          <!-- 崩溃后 5 秒重启 -->
<key>ProcessType</key>
<string>Background</string>
<key>LowPriorityBackgroundIO</key>
<true/>
```

### 3.25 OTA 更新系统 (`clawx-ota`) 扩展 — v3.0 新增

```rust
pub struct UpdateManifest {
    pub version: String,
    pub channel: UpdateChannel,
    pub release_date: DateTime<Utc>,
    pub changelog: String,
    pub delta_url: Option<String>,         // bsdiff 补丁 URL
    pub delta_size: Option<u64>,
    pub full_url: String,                  // 完整包 URL
    pub full_size: u64,
    pub signature: String,                 // Ed25519 签名
    pub min_os_version: String,
    pub is_security_patch: bool,           // 紧急安全补丁标记
}

pub enum UpdateChannel {
    Stable,
    Beta,
    Nightly,
}

pub enum UpdateStatus {
    NoUpdate,
    Available(UpdateManifest),
    Downloading { progress_percent: u8 },
    ReadyToInstall,
    Installing,
    Installed { requires_restart: bool },
    Failed(String),
}

#[async_trait]
pub trait UpdateEngine: Send + Sync {
    /// 检查更新（定期自动或用户手动触发）
    async fn check_update(&self, channel: &UpdateChannel) -> Result<UpdateStatus>;

    /// 下载更新（优先 delta，降级为 full）
    async fn download(&self, manifest: &UpdateManifest) -> Result<PathBuf>;

    /// 验证更新包（Ed25519 签名 + SHA256 校验）
    async fn verify(&self, package_path: &Path, manifest: &UpdateManifest) -> Result<bool>;

    /// 应用更新（写入 staging 分区）
    async fn apply(&self, package_path: &Path) -> Result<()>;

    /// 回滚到上一版本（切换回 previous 分区）
    async fn rollback(&self) -> Result<()>;

    /// 获取更新历史
    async fn history(&self) -> Result<Vec<UpdateRecord>>;

    /// 配置更新通道和检查频率
    async fn configure(&self, channel: UpdateChannel, check_interval: Duration) -> Result<()>;
}
```

**A/B 分区方案**：
```
~/.clawx/
  versions/
    current/      → symlink 到 v1.2.3/
    staging/       → 新版本写入此处
    previous/      → 旧版本保留用于回滚
    v1.2.3/
    v1.2.4/
```

**更新流程**（AD-22）：
```
定期检查 → UpdateManifest 可用
  → 安全补丁 (is_security_patch=true) → 静默下载 + 后台安装
  → 常规更新 → 通知用户 → 用户确认 → 下载
  → 优先下载 delta (bsdiff) → 失败则降级为 full
  → Ed25519 签名验证 → SHA256 校验
  → 写入 staging 分区 → 重启进入 staging
  → 健康检查通过 → 提升为 current，旧版本移到 previous
  → 健康检查失败 → 自动回滚到 previous
```

### 3.26 基础设置系统 (`clawx-settings`) — v3.0 新增

```rust
pub enum ThemeMode { Light, Dark, System }

pub struct ThemeConfig {
    pub mode: ThemeMode,
    pub accent_color: Option<String>,      // hex color
}

pub enum Locale {
    En,
    ZhCn,
}

pub struct SettingsSchema {
    pub key: String,
    pub value_type: SettingsValueType,
    pub default_value: serde_json::Value,
    pub description: String,
}

pub enum SettingsValueType { String, Bool, Integer, Float, Enum(Vec<String>) }

#[async_trait]
pub trait SettingsStore: Send + Sync {
    /// 获取设置值
    async fn get(&self, key: &str) -> Result<serde_json::Value>;

    /// 设置值（自动验证类型）
    async fn set(&self, key: &str, value: serde_json::Value) -> Result<()>;

    /// 重置为默认值
    async fn reset_to_default(&self, key: &str) -> Result<()>;

    /// 导出所有设置（用于备份）
    async fn export(&self) -> Result<serde_json::Value>;

    /// 导入设置（用于恢复）
    async fn import(&self, settings: serde_json::Value) -> Result<()>;

    /// 获取当前语言
    async fn get_locale(&self) -> Result<Locale>;

    /// 设置语言
    async fn set_locale(&self, locale: Locale) -> Result<()>;

    /// 获取主题配置
    async fn get_theme(&self) -> Result<ThemeConfig>;

    /// 设置主题
    async fn set_theme(&self, theme: ThemeConfig) -> Result<()>;
}
```

**i18n 架构**（AD-23）：

采用 Mozilla Fluent 格式 (`fluent-rs` crate)：

```
locales/
  en/
    main.ftl
    agents.ftl
    settings.ftl
  zh-CN/
    main.ftl
    agents.ftl
    settings.ftl
```

**示例 `locales/en/agents.ftl`**：
```fluent
agent-create = Create Agent
agent-delete-confirm = Are you sure you want to delete "{$name}"?
agent-status-idle = Idle
agent-status-working = Working
agent-template-pm = Project Manager
```

**示例 `locales/zh-CN/agents.ftl`**：
```fluent
agent-create = 创建 Agent
agent-delete-confirm = 确定要删除"{$name}"吗？
agent-status-idle = 空闲
agent-status-working = 工作中
agent-template-pm = 项目经理
```

**Swift 侧集成**：通过 UniFFI 暴露 `fn localize(key: &str, args: HashMap<String, String>) -> String`，SwiftUI 通过此方法获取本地化字符串。

---

## 4. 数据流

### 4.1 GUI 用户消息流

```
SwiftUI View → ViewModel → UniFFI bridge → clawx-ffi
  → clawx-gateway::authenticate()
  → clawx-runtime::dispatch(agent_id, message)
  → Agent Loop:
       → clawx-security::prompt_injection_scan(input)
       → clawx-memory::smart_inject(相关记忆)
       → clawx-kb::search(相关知识)
       → 构建 Prompt (system + memories + knowledge + user input)
       → clawx-llm::complete(prompt)
       → 解析响应:
            ToolCall → security::authorize → vault::create_version_point
                     → skills::execute → security::dlp_scan
                     → artifact::register (如生成文件) → 循环
            TextOutput → security::dlp_scan → memory::store(摘要) → 返回
  → UsageAggregator::record(usage)
  → UniFFI bridge (callback/stream) → SwiftUI 更新
```

### 4.2 IM 渠道消息流

```
IM 平台 (如 Telegram)
  → clawx-channel::TelegramAdapter.receive_stream()
  → clawx-gateway::resolve_agent(Binding Rules 路由)
  → clawx-gateway::enqueue(agent_id, message)  → Session Lane 队列
  → clawx-runtime::dispatch(agent_id, message)
  → [同 4.1 的 Agent Loop]
  → clawx-channel::send_message(response)
```

### 4.3 主动式 Agent 流

```
clawx-scheduler:
  Cron 触发 | EventBus 事件 | 上下文感知检查 | 策略匹配
    → clawx-runtime::dispatch(agent_id, task_prompt)
    → [Agent Loop]
    → clawx-scheduler::notify_result():
        → IM 渠道推送 (clawx-channel::send_message)
        → macOS 桌面通知 (clawx-ffi → NSUserNotification)
        → 移动端推送 (clawx-mobile-relay::push_notification)
    → 等待用户反馈 → clawx-scheduler::record_feedback()
    → 负反馈率 > 15% → 自动降低频率或暂停
```

### 4.4 数据保险箱版本化/还原流

```
版本点创建（自动）:
  Agent 执行文件写入/删除/移动/重命名
    → Hook: before_tool_call
    → clawx-vault::create_version_point(agent_id, task_id)
    → clawx-vault::store_blob(原始文件内容) → SHA256 去重
    → 执行文件操作
    → clawx-vault::record_change(version_id, FileChange)
    → EventBus::publish(VersionPointCreated)

还原（用户触发）:
  用户在 GUI 选择版本点
    → clawx-vault::diff_version(version_id) → DiffPreview
    → GUI 展示差异（绿/黄/红标注）
    → 用户确认
    → 文件级还原: clawx-vault::restore_file(version_id, path)
      或任务级回滚: clawx-vault::restore_task(task_id)
    → clawx-vault::read_blob(hash) → 恢复文件内容
    → EventBus::publish(RestoreCompleted)

智能清理:
  clawx-daemon 定期触发
    → clawx-vault::cleanup(RetentionPolicy)
    → 7 天内全保留
    → 7-30 天每天保留 1 个
    → 30 天以上自动删除
    → blob ref_count == 0 时删除 blob 文件
    → 磁盘 < 10% 时弹窗警告
```

### 4.5 账号登录流

```
用户点击登录 → SwiftUI → clawx-ffi → clawx-account::AuthProvider::login(flow)
  → OAuth 重定向 → 系统浏览器打开 Provider 页面
  → 用户授权 → 回调 URL → 获取 authorization code
  → 交换 access_token + refresh_token
  → Token 存入 macOS Keychain (keychain_ref)
  → Session 写入 account.db
  → 返回 Session → GUI 更新登录状态

云端功能调用:
  clawx-sync/community/skills-store 调用
    → AccountGate::require_session()
    → Session 存在且未过期 → 通过
    → Session 过期 → AuthProvider::refresh_token() → 更新 Keychain
    → Session 不存在 → 返回 ClawxError::AccountRequired → GUI 提示登录
```

### 4.6 云端备份/还原流

```
创建备份:
  clawx-sync::create_backup(target, contents)
    → AccountGate::require_session()
    → 收集数据: Agent 配置 + 记忆 + Skills 设置 + 偏好
    → 序列化为 JSON → 压缩 (zstd)
    → 用户密钥从 Keychain 读取 → AES-256-GCM 加密
    → 计算 SHA256 checksum
    → 增量 diff（与上次备份比较，仅上传变更）
    → 上传到 SyncTarget (iCloud/GDrive/WebDAV)
    → 存储 BackupManifest 到 account.db

还原备份:
  新设备安装 ClawX
    → 登录账号 (clawx-account)
    → clawx-sync::list_backups(target) → 展示备份列表
    → 用户选择备份
    → 下载加密数据 → 验证 checksum
    → 用户输入密钥 → AES-256-GCM 解密 → 解压
    → 应用: Agent 配置 → core.db, 记忆 → memory.db, Skills → skills 表
    → 知识库: 提示用户迁移原始文件 → 重新构建索引
    → RestoreReport: 统计恢复数量 + 警告（版本不兼容等）
```

### 4.7 物理设备事件流

```
设备发现:
  clawx-physical::DeviceAdapter::discover()
    → mDNS/SSDP 局域网扫描 → 返回 Vec<DeviceInfo>
    → GUI 展示发现的设备 → 用户选择接入
    → DeviceAdapter::connect(device_id)
    → EventBus::publish(DeviceConnected)

设备事件触发 Agent:
  Camera RTSP Stream → CameraAdapter 帧分析 (ONNX via clawx-hal)
    → PersonDetected { confidence: 0.95, snapshot_path }
    → EventBus::publish(DeviceEvent)
    → clawx-scheduler 匹配策略 (PolicyMatch)
    → 触发 Agent 任务: "检测到陌生人，截图并通知"
    → Agent Loop:
        → CameraAdapter::capture_frame() → 保存截图
        → ArtifactStore::register(screenshot)
        → ChannelAdapter::send_message(Telegram, "检测到异常 + 截图")
        → SmartHomeAdapter::control("living-room-light", "turn_on")
        → SecurityGate: CapabilityKind::DeviceControl 检查

对话式控制:
  用户: "关掉卧室的灯"
    → Agent Loop → LLM function-call: control_device("bedroom-light", "turn_off")
    → SecurityGate::authorize(DeviceControl) → 首次需确认，后续自动
    → SmartHomeAdapter::control("bedroom-light", { power: "off" })
    → 返回结果: "卧室灯已关闭"
```

### 4.8 移动端随行流

```
首次配对:
  Mac 端: clawx-mobile-relay::generate_pairing_qr()
    → 生成 QR 码 (含 WireGuard peer config + 临时 ECDH 公钥)
    → GUI 展示 QR 码
  iOS 端: 扫描 QR 码 → 建立 WireGuard 隧道
    → ECDH 密钥交换 → 生成 mTLS 证书对
    → clawx-mobile-relay::complete_pairing()
    → 配对信息存储 → 后续连接自动验证

日常使用:
  iOS App → WireGuard 隧道 → clawx-mobile-relay::RelayServer
    → authenticate_client(mTLS cert)
    → gRPC ChatRequest { agent_id, message }
    → clawx-gateway → Agent Loop → 流式响应
    → gRPC ChatResponseChunk (streaming) → iOS App 实时显示

通知推送:
  EventBus 事件 (TaskCompleted, ProactiveResult, DeviceAlert)
    → clawx-mobile-relay::push_notification()
    → gRPC NotificationPush → iOS App
    → iOS 系统通知
```

### 4.9 OTA 更新流

```
自动检查:
  clawx-daemon → 定期 (默认每天) → clawx-ota::check_update(channel)
    → HTTP GET update-server/manifest.json → 解析 UpdateManifest
    → 比较版本号

安全补丁 (is_security_patch = true):
  → 静默下载 delta/full 包
  → verify(Ed25519 + SHA256) → apply() → 写入 staging
  → 下次重启自动生效

常规更新:
  → EventBus::publish(UpdateAvailable)
  → GUI 弹窗: 展示 changelog → 用户确认
  → download(): 优先 delta (bsdiff) → 失败降级 full
  → verify(): Ed25519 签名 + SHA256 校验
  → apply(): 解压到 staging 分区
  → 重启进入 staging
  → Watchdog 健康检查 (30 秒):
      → 通过 → promote staging → current, current → previous
      → 失败 → rollback() → 切回 previous 分区
  → EventBus::publish(UpdateApplied)
```

### 4.10 OpenClaw 迁移流

```
clawx-migration::detect()
  → 扫描常见 OpenClaw 安装路径 (~/.openclaw, ~/openclaw 等)
  → 检测到 → 返回 MigrationSource::OpenClaw { path }

clawx-migration::plan(source)
  → 解析 OpenClaw 数据库 / 配置文件
  → 列出可迁移项: agents, conversations, skills, kb data, api keys
  → 标注兼容性问题 (如 Skills 格式不兼容)
  → 返回 MigrationPlan → GUI 展示预览

用户确认 → clawx-migration::execute(plan)
  → clawx-vault::create_version_point("migration-rollback") → 回滚点
  → 按实体类型逐一迁移:
      → Agent 配置: JSON → agents 表
      → 对话历史: 格式转换 → conversations + messages 表
      → API Keys: 明文/环境变量 → macOS Keychain
      → 知识库: 记录原始文件路径 → 标记需重新索引
      → Skills: 检查 WASM 版本可用性 → 安装或标记不兼容
  → 迁移完成 → 验证数据完整性

失败回滚:
  → clawx-migration::rollback(plan)
  → clawx-vault::restore_task(migration-rollback) → 恢复原状
```

### 4.11 Skills 商店安装流

```
用户浏览商店:
  → clawx-skills::SkillsStoreClient::browse(query)
  → AccountGate::require_session() (商店需登录)
  → HTTP API → 返回 Vec<SkillListing>
  → GUI 展示: 名称、评分、下载量、定价

安装 Skill:
  → 免费: 直接下载 | 付费: purchase() → 支付 → 获取 SkillLicense
  → download_and_install(skill_id)
  → 下载 WASM bundle
  → Ed25519 签名验证 (manifest + WASM binary)
  → 四层安全检测:
      1. 静态审计: clawx-security 扫描 WASM 字节码
      2. 沙箱测试: T1 运行，监控 Fuel 消耗和系统调用
      3. 权限最小化: 验证声明权限与实际行为一致
      4. 审计日志: 记录安装行为到 Merkle 审计链
  → 安装到 ~/.clawx/wasm/{skill_id}/
  → clawx-vault::create_skill_install_point(skill_id)
  → skills 表写入 → agent_skills 表绑定
  → EventBus::publish(SkillInstalled)
```

---

## 5. 数据库 Schema

所有数据库使用 SQLite WAL 模式。按领域分离数据库文件以减少写竞争。

### 5.1 核心数据库 (`~/.clawx/data/core.db`)

```sql
-- Agent 配置
CREATE TABLE agents (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    role          TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    model_provider TEXT NOT NULL,
    model_name    TEXT NOT NULL,
    model_params  TEXT,              -- JSON: ModelParams
    status        TEXT NOT NULL DEFAULT 'idle',  -- idle|working|error|offline
    icon          TEXT,              -- 角色图标标识
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

-- Agent-Skill 绑定
CREATE TABLE agent_skills (
    agent_id  TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    skill_id  TEXT NOT NULL REFERENCES skills(id) ON DELETE CASCADE,
    enabled   INTEGER NOT NULL DEFAULT 1,
    config    TEXT,                  -- JSON: Skill 级别参数覆盖
    PRIMARY KEY (agent_id, skill_id)
);

-- Agent-Channel 绑定
CREATE TABLE agent_channels (
    agent_id    TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    channel_id  TEXT NOT NULL,
    platform    TEXT NOT NULL,       -- feishu|telegram|slack|whatsapp|discord|wecom
    config      TEXT NOT NULL,       -- JSON: 渠道配置
    status      TEXT NOT NULL DEFAULT 'disconnected',
    PRIMARY KEY (agent_id, channel_id)
);

-- 任务
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

-- 定时/主动任务
CREATE TABLE scheduled_jobs (
    id              TEXT PRIMARY KEY,
    agent_id        TEXT NOT NULL REFERENCES agents(id),
    name            TEXT NOT NULL,
    trigger_type    TEXT NOT NULL,   -- cron|event_driven|context_aware|policy_match
    trigger_config  TEXT NOT NULL,   -- JSON: TriggerType 序列化
    task_prompt     TEXT NOT NULL,
    enabled         INTEGER NOT NULL DEFAULT 1,
    last_run        TEXT,
    next_run        TEXT,
    notify_channels TEXT,            -- JSON array of ChannelId
    notify_desktop  INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL
);

-- Skills
CREATE TABLE skills (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    version        TEXT NOT NULL,
    description    TEXT,
    author         TEXT,
    manifest       TEXT NOT NULL,     -- JSON: SkillManifest
    wasm_path      TEXT,
    execution_tier TEXT NOT NULL DEFAULT 'sandboxed',
    signature      TEXT,              -- Ed25519 签名
    installed_at   TEXT NOT NULL,
    source         TEXT               -- store|local|opensource
);

-- Skill 许可证
CREATE TABLE skill_licenses (
    skill_id      TEXT PRIMARY KEY REFERENCES skills(id) ON DELETE CASCADE,
    license_type  TEXT NOT NULL,      -- free|purchased|subscription|rental
    purchased_at  TEXT NOT NULL,
    expires_at    TEXT,
    receipt       TEXT NOT NULL       -- 加密购买凭证
);

-- 模型 Provider 注册
CREATE TABLE model_providers (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    provider_type  TEXT NOT NULL,     -- openai|anthropic|ollama|custom
    base_url       TEXT,
    keychain_ref   TEXT NOT NULL,     -- macOS Keychain 引用
    rotation_policy TEXT,             -- JSON: KeyRotationPolicy
    default_model  TEXT NOT NULL,
    default_params TEXT,              -- JSON: ModelParams
    last_rotated   TEXT,
    created_at     TEXT NOT NULL
);

-- 文件产物
CREATE TABLE artifacts (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id),
    task_id     TEXT,
    filename    TEXT NOT NULL,
    mime_type   TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    path        TEXT NOT NULL,
    preview_path TEXT,
    tags        TEXT,                 -- JSON array
    created_at  TEXT NOT NULL
);

-- 物理设备
CREATE TABLE devices (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    device_type TEXT NOT NULL,        -- camera|light|switch|sensor|thermostat|lock|speaker
    protocol    TEXT NOT NULL,        -- rtsp|onvif|mqtt|homekit|mihome
    address     TEXT NOT NULL,
    config      TEXT,                 -- JSON: 设备特定配置
    status      TEXT NOT NULL DEFAULT 'disconnected',
    capabilities TEXT,                -- JSON array: DeviceCapability
    created_at  TEXT NOT NULL
);

-- 设备自动化规则
CREATE TABLE device_automations (
    id              TEXT PRIMARY KEY,
    device_id       TEXT NOT NULL REFERENCES devices(id) ON DELETE CASCADE,
    trigger_event   TEXT NOT NULL,    -- JSON: DeviceEventPayload 匹配规则
    agent_id        TEXT NOT NULL REFERENCES agents(id),
    task_prompt     TEXT NOT NULL,
    notify_channels TEXT,             -- JSON array
    enabled         INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL
);

-- 设置
CREATE TABLE settings (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,        -- JSON
    updated_at  TEXT NOT NULL
);

-- Binding Rules (渠道路由)
CREATE TABLE binding_rules (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id) ON DELETE CASCADE,
    priority    TEXT NOT NULL,        -- peer|guild|account|channel
    matcher     TEXT NOT NULL,        -- JSON: BindingMatcher
    created_at  TEXT NOT NULL
);
```

### 5.2 记忆数据库 (`~/.clawx/data/memory.db`)

```sql
-- Agent 记忆（仅所属 Agent 可读写）
CREATE TABLE agent_memories (
    id            TEXT PRIMARY KEY,
    agent_id      TEXT NOT NULL,
    kind          TEXT NOT NULL,      -- fact|preference|event|skill
    summary       TEXT NOT NULL,
    detail        TEXT,               -- JSON
    importance    REAL NOT NULL DEFAULT 5.0,
    freshness     REAL NOT NULL DEFAULT 10.0,
    pinned        INTEGER NOT NULL DEFAULT 0,
    access_count  INTEGER NOT NULL DEFAULT 0,
    created_at    TEXT NOT NULL,
    last_accessed TEXT NOT NULL,
    embedding_id  TEXT
);
CREATE INDEX idx_agent_memories_agent ON agent_memories(agent_id);
CREATE INDEX idx_agent_memories_freshness ON agent_memories(freshness);

-- 用户记忆（全局共享，所有 Agent 可读写）
CREATE TABLE user_memories (
    id            TEXT PRIMARY KEY,
    kind          TEXT NOT NULL,      -- fact|preference|event|skill
    summary       TEXT NOT NULL,
    detail        TEXT,               -- JSON
    importance    REAL NOT NULL DEFAULT 5.0,
    freshness     REAL NOT NULL DEFAULT 10.0,
    pinned        INTEGER NOT NULL DEFAULT 0,
    access_count  INTEGER NOT NULL DEFAULT 0,
    source_agent  TEXT,               -- 写入此记忆的 Agent ID
    confirmed     INTEGER NOT NULL DEFAULT 0,  -- 用户是否确认
    created_at    TEXT NOT NULL,
    last_accessed TEXT NOT NULL,
    embedding_id  TEXT
);

-- 对话
CREATE TABLE conversations (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL,
    channel_id  TEXT,                 -- NULL 表示 GUI 对话
    title       TEXT,                 -- 自动生成的标题
    archived    INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL,
    summary     TEXT
);
CREATE INDEX idx_conversations_agent ON conversations(agent_id);

-- 消息
CREATE TABLE messages (
    id              TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,    -- user|assistant|system|tool
    content         TEXT NOT NULL,
    tool_call_id    TEXT,
    tool_name       TEXT,
    token_count     INTEGER,
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_messages_conversation ON messages(conversation_id);
```

### 5.3 知识库数据库 (`~/.clawx/data/knowledge.db`)

```sql
-- 知识源文件夹
CREATE TABLE kb_sources (
    id          TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    status      TEXT NOT NULL DEFAULT 'pending',  -- pending|indexing|ready|error
    file_count  INTEGER NOT NULL DEFAULT 0,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    last_indexed TEXT,
    config      TEXT                   -- JSON: IndexConfig
);

-- 已索引文件
CREATE TABLE kb_files (
    id          TEXT PRIMARY KEY,
    source_id   TEXT NOT NULL REFERENCES kb_sources(id) ON DELETE CASCADE,
    path        TEXT NOT NULL,
    hash        TEXT NOT NULL,          -- 文件内容 SHA256，用于增量更新
    format      TEXT NOT NULL,
    size_bytes  INTEGER NOT NULL,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'pending',
    indexed_at  TEXT,
    error       TEXT
);
CREATE INDEX idx_kb_files_source ON kb_files(source_id);
CREATE INDEX idx_kb_files_hash ON kb_files(hash);

-- 文本块
CREATE TABLE kb_chunks (
    id           TEXT PRIMARY KEY,
    file_id      TEXT NOT NULL REFERENCES kb_files(id) ON DELETE CASCADE,
    content      TEXT NOT NULL,
    chunk_index  INTEGER NOT NULL,
    token_count  INTEGER NOT NULL,
    embedding_id TEXT,                  -- Qdrant point ID
    metadata     TEXT                   -- JSON: 段落标题、页码等
);
CREATE INDEX idx_kb_chunks_file ON kb_chunks(file_id);
```

### 5.4 审计数据库 (`~/.clawx/data/audit.db`)

```sql
-- Merkle 审计链
CREATE TABLE audit_log (
    id         TEXT PRIMARY KEY,
    timestamp  TEXT NOT NULL,
    agent_id   TEXT,
    module     TEXT NOT NULL,          -- runtime|security|channel|skills|vault|scheduler|physical
    action     TEXT NOT NULL,
    severity   TEXT NOT NULL,          -- info|warning|critical
    detail     TEXT NOT NULL,          -- JSON
    prev_hash  TEXT NOT NULL,          -- 前一条的 hash（首条为空字符串）
    hash       TEXT NOT NULL           -- SHA256(prev_hash + detail)
);
CREATE INDEX idx_audit_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_agent ON audit_log(agent_id);
CREATE INDEX idx_audit_severity ON audit_log(severity);

-- 用量统计
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
CREATE INDEX idx_usage_agent ON usage_stats(agent_id);
CREATE INDEX idx_usage_timestamp ON usage_stats(timestamp);

-- 主动任务反馈
CREATE TABLE proactive_feedback (
    id          TEXT PRIMARY KEY,
    task_id     TEXT NOT NULL,
    feedback    TEXT NOT NULL,          -- adopted|dismissed|reduce_frequency|never_again
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_feedback_task ON proactive_feedback(task_id);

-- 迁移历史
CREATE TABLE migration_log (
    id                TEXT PRIMARY KEY,
    source            TEXT NOT NULL,    -- openclaw
    source_path       TEXT NOT NULL,
    status            TEXT NOT NULL,    -- planned|in_progress|completed|rolled_back
    plan              TEXT NOT NULL,    -- JSON: MigrationPlan
    vault_snapshot_id TEXT,             -- 回滚用版本点 ID
    started_at        TEXT NOT NULL,
    completed_at      TEXT
);

-- OTA 更新历史
CREATE TABLE update_history (
    id          TEXT PRIMARY KEY,
    from_version TEXT NOT NULL,
    to_version  TEXT NOT NULL,
    channel     TEXT NOT NULL,          -- stable|beta|nightly
    update_type TEXT NOT NULL,          -- delta|full|security_patch
    status      TEXT NOT NULL,          -- downloaded|applied|rolled_back|failed
    changelog   TEXT,
    applied_at  TEXT NOT NULL
);
```

### 5.5 版本管理数据库 (`~/.clawx/data/vault.db`) — v3.0 新增

```sql
-- 版本点
CREATE TABLE version_points (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT,
    task_id     TEXT,
    label       TEXT NOT NULL,         -- clawx-{agent_id}-{task_id}-{timestamp}
    description TEXT,
    point_type  TEXT NOT NULL DEFAULT 'task',  -- task|skill_install|migration|manual
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_vp_agent ON version_points(agent_id);
CREATE INDEX idx_vp_task ON version_points(task_id);
CREATE INDEX idx_vp_created ON version_points(created_at);

-- 变更集
CREATE TABLE changesets (
    id              TEXT PRIMARY KEY,
    version_id      TEXT NOT NULL REFERENCES version_points(id) ON DELETE CASCADE,
    file_path       TEXT NOT NULL,     -- 工作区内相对路径
    change_type     TEXT NOT NULL,     -- added|modified|deleted|renamed
    before_blob_hash TEXT,             -- SHA256, NULL for 'added'
    after_blob_hash  TEXT,             -- SHA256, NULL for 'deleted'
    before_size     INTEGER,
    after_size      INTEGER,
    rename_from     TEXT               -- 仅 'renamed' 类型使用
);
CREATE INDEX idx_cs_version ON changesets(version_id);
CREATE INDEX idx_cs_file ON changesets(file_path);

-- 内容寻址 blob 元数据（实际 blob 存储在 ~/.clawx/vault/blobs/）
CREATE TABLE blobs (
    hash        TEXT PRIMARY KEY,      -- SHA256
    size_bytes  INTEGER NOT NULL,
    ref_count   INTEGER NOT NULL DEFAULT 1,  -- 引用计数，0 时可删除
    created_at  TEXT NOT NULL
);
```

### 5.6 账号数据库 (`~/.clawx/data/account.db`) — v3.0 新增

```sql
-- 登录会话
CREATE TABLE sessions (
    id            TEXT PRIMARY KEY,
    user_id       TEXT NOT NULL,
    provider      TEXT NOT NULL,       -- google|apple|email|phone
    access_token_ref TEXT NOT NULL,    -- Keychain 引用
    refresh_token_ref TEXT,            -- Keychain 引用
    expires_at    TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

-- 用户资料缓存
CREATE TABLE user_profiles (
    user_id       TEXT PRIMARY KEY,
    display_name  TEXT NOT NULL,
    email         TEXT,
    avatar_url    TEXT,
    updated_at    TEXT NOT NULL
);

-- 备份清单
CREATE TABLE backup_manifests (
    id                TEXT PRIMARY KEY,
    target            TEXT NOT NULL,    -- icloud|gdrive|webdav
    target_config     TEXT NOT NULL,    -- JSON: SyncTarget 配置
    version           TEXT NOT NULL,    -- ClawX 版本号
    content_list      TEXT NOT NULL,    -- JSON array: BackupContent
    content_hash      TEXT NOT NULL,    -- SHA256
    encryption_key_ref TEXT NOT NULL,   -- Keychain 引用
    size_bytes        INTEGER NOT NULL,
    status            TEXT NOT NULL,    -- created|uploading|uploaded|failed
    created_at        TEXT NOT NULL
);

-- 增量同步状态
CREATE TABLE sync_state (
    entity_type  TEXT NOT NULL,        -- agent|memory|skill|setting|scheduled_job
    entity_id    TEXT NOT NULL,
    local_version INTEGER NOT NULL,
    remote_version INTEGER,
    last_synced  TEXT,
    PRIMARY KEY (entity_type, entity_id)
);

-- 已配对移动设备
CREATE TABLE paired_devices (
    device_id     TEXT PRIMARY KEY,
    device_name   TEXT NOT NULL,
    public_key    BLOB NOT NULL,       -- mTLS 公钥
    platform      TEXT NOT NULL,       -- ios|android
    paired_at     TEXT NOT NULL,
    last_seen     TEXT
);
```

---

## 6. 跨切面关注点

### 6.1 错误处理

统一错误类型定义在 `clawx-types`：

```rust
#[derive(Debug, thiserror::Error)]
pub enum ClawxError {
    // LLM
    #[error("LLM provider error: {provider}: {message}")]
    LlmProvider { provider: String, message: String },
    #[error("LLM rate limited, retry after {retry_after_secs}s")]
    LlmRateLimited { retry_after_secs: u64 },

    // 安全
    #[error("Tool execution denied: {reason}")]
    SecurityDenied { reason: String },
    #[error("DLP violation: {pattern} in {direction}")]
    DlpViolation { pattern: String, direction: String },
    #[error("Prompt injection detected")]
    PromptInjection { details: String },

    // 数据
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    // WASM
    #[error("WASM execution error: {0}")]
    WasmExecution(String),
    #[error("WASM timeout after {timeout_secs}s")]
    WasmTimeout { timeout_secs: u64 },

    // 渠道
    #[error("Channel {platform} error: {message}")]
    ChannelConnection { platform: String, message: String },

    // 配置
    #[error("Config error: {0}")]
    Config(String),

    // Vault (v3.0 新增)
    #[error("Vault error: {0}")]
    Vault(String),

    // 账号 (v3.0 新增)
    #[error("Account required for {feature}. Please login first.")]
    AccountRequired { feature: String },

    // 同步 (v3.0 新增)
    #[error("Sync error with {target}: {message}")]
    SyncError { target: String, message: String },

    // 设备 (v3.0 新增)
    #[error("Device {device_type} error: {message}")]
    DeviceError { device_type: String, message: String },

    // 迁移 (v3.0 新增)
    #[error("Migration error in {phase}: {message}")]
    MigrationError { phase: String, message: String },

    // OTA (v3.0 新增)
    #[error("Update error: {0}")]
    UpdateError(String),

    // 通用
    #[error("Internal: {0}")]
    Internal(#[from] anyhow::Error),
}
```

### 6.2 日志

所有 crate 使用 `tracing` + 结构化字段。安全规则：INFO 及以上**永不**记录 API Key、密码、PII、完整 Prompt。

### 6.3 配置目录结构

```
~/.clawx/
  config/
    clawx.toml               # 主配置
    network-whitelist.toml    # 网络白名单
    skills/                   # 每 Skill 配置覆盖
  data/
    core.db                   # Agent、Skills、设备、设置
    memory.db                 # 记忆、对话
    knowledge.db              # 知识库索引元数据
    audit.db                  # 审计、用量、反馈、迁移、OTA 历史
    vault.db                  # 版本点、changeset、blob 元数据
    account.db                # 会话、备份、同步、配对设备
  qdrant/                     # Qdrant 嵌入式数据
  tantivy/                    # Tantivy 全文索引
  vault/
    blobs/                    # 内容寻址 blob 存储 (SHA256 分级目录)
  workspace/                  # ClawX 受管工作区（版本化保护范围）
    imports/                  # 对话拖拽文件的受管副本
  artifacts/                  # Agent 生成的文件产物
  wasm/                       # 已安装 WASM Skill 二进制
  versions/                   # OTA 更新 A/B 分区
    current/
    staging/
    previous/
  logs/                       # tracing 输出
  locales/                    # i18n 覆盖（用户自定义翻译）
```

### 6.4 数据库迁移

使用 `sqlx::migrate!()`，迁移文件命名：`YYYYMMDDHHMMSS_description.sql`。守护进程启动时自动执行迁移。永不在迁移中删除列，仅标记废弃。

### 6.5 国际化 (i18n)

- 格式：Mozilla Fluent (`.ftl` 文件)
- 引擎：`fluent-rs` crate
- 默认语言：英文 (en)，初期支持简体中文 (zh-CN)
- 翻译包位置：`locales/{locale}/` (编译时嵌入) + `~/.clawx/locales/{locale}/` (用户覆盖)
- 编译时验证：确保所有 key 在所有 locale 中都有定义
- Swift 侧集成：UniFFI 暴露 `localize(key, args)` 函数，SwiftUI 通过此方法获取字符串

### 6.6 数据脱敏上云

当数据需要传输到云端 LLM API 时（对应 PRD §2.5.3）：
1. 出站前扫描 PII（姓名、邮箱、电话、身份证号等）
2. 替换为占位符：`[PII:name:1]`、`[PII:email:2]`
3. 本地维护占位符 → 原始值映射表
4. LLM 响应返回后，扫描占位符并还原为原始值
5. 映射表仅存内存，会话结束后销毁

---

## 7. 路线图

基于 PRD v2.0 §4.1 功能优先级表，映射到 crate 交付：

| 阶段 | 版本 | 周期 | 交付 crate | 交付能力 |
|------|------|------|-----------|---------|
| **奠基** | v0.1 | 14 周 | types, config, eventbus, llm (含 ModelRegistry), runtime (Agent Loop + Hook + 循环检测), memory (两层记忆 + Flush), vault (工作区版本化), security (基础 DLP + 路径限制 + 凭证注入 + Merkle 审计), settings (基础 i18n + 主题), ffi | 单 Agent GUI 对话、模型配置、记忆中心、工作区回滚、安全基线、基础设置 |
| **生态** | v0.2 | 12 周 | security (完整 WASM 沙箱 + 双计量), skills (WASM + WIT + 本地管理), channel (飞书+Telegram+Slack), gateway (Binding Rules + Lane), scheduler (Cron + Hands + NL-to-Cron), kb (混合检索 + FSEvents) | 多 Agent、Skills 本地管理、IM 渠道、主动式 Agent、知识库引擎 |
| **加固** | v0.3 | 10 周 | daemon (看门狗 + 心跳 + 内存监控), artifact (产物管理), account (OAuth 登录), migration (OpenClaw 迁移), llm (UsageAggregator), api | 完整系统稳定性、文件产物、账号体系、用量统计、迁移、REST API |
| **更新** | v0.4 | 8 周 | ota (Delta + A/B + 签名), hal (ONNX Runtime) | OTA 自动更新、硬件加速推理 |
| **扩展** | v0.5 | 8 周 | skills (商店客户端 + 许可证), sync (加密备份 + 增量同步), physical (摄像头 + 智能家居) | Skills 商店、云端备份、物理 Agent 接入、Agent 分享 |
| **社区** | v1.0 | 10 周 | community (HireClaw), mobile-relay (gRPC + WireGuard) | HireClaw 社区、移动端随行、Agent 商业化 |

**关键依赖链**：
- v0.1 是独立闭环，不依赖任何后续阶段
- v0.2 依赖 v0.1 的 runtime + security + eventbus
- v0.3 的 account 是 v0.5 sync/community/skills-store 的前置
- v0.5 的 physical 依赖 v0.2 的 eventbus + scheduler

---

## 8. Hands 自治 Agent 包（来源: OpenFang AD-15）

Hands 是 ClawX 实现"主动式 Agent"的核心模式。每个 Hand 是一个自包含的自治 Agent 包。

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
| 分发 | 不可分发 | 可打包分享到 HireClaw 社区 |

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
| 11 | Vault blob 去重策略 | 跨 Agent 全局去重（同一 blob store） | v0.1 开始前 |
| 12 | 移动端隧道方案 | 默认 Tailscale（零配置），可选 WireGuard（自建） | v1.0 开始前 |
| 13 | HireClaw 社区后端 | 自建后端（完整控制），初期可用 Supabase 加速 | v1.0 开始前 |
| 14 | 物理设备云 API | 默认仅本地协议（RTSP/MQTT/HomeKit），云 API 需用户显式启用 | v0.5 开始前 |
| 15 | 备份加密密钥托管 | 仅用户持有密钥，无托管（丢失密钥 = 无法恢复） | v0.5 开始前 |

---

## 10. PRD-to-Architecture 追溯矩阵

此矩阵确保 PRD v2.0 的**每一个功能点**都映射到具体的 crate、trait、数据库表和数据流。

| PRD 章节 | 功能 | Crate(s) | 核心 Trait | DB 表 | 数据流 | 阶段 |
|---------|------|----------|-----------|-------|--------|------|
| **2.1** Agent 工作台 | Agent CRUD | clawx-runtime, clawx-ffi | AgentExecutor | agents | 4.1 | v0.1 |
| 2.1 | 流式对话 | clawx-llm, clawx-runtime | LlmProvider (complete_stream) | conversations, messages | 4.1 | v0.1 |
| 2.1 | 多轮对话 + 摘要压缩 | clawx-runtime, clawx-memory | AgentHook (on_compaction) | messages | 4.1 | v0.1 |
| 2.1 | 富文本消息 | clawx-ffi (SwiftUI 渲染) | — | messages | 4.1 | v0.1 |
| 2.1 | 消息操作 (重新生成/编辑/复制/收藏) | clawx-runtime, clawx-ffi | — | messages | 4.1 | v0.1 |
| 2.1 | 对话管理 (新建/归档/删除/搜索) | clawx-runtime | — | conversations | 4.1 | v0.1 |
| 2.1 | 文件交互 (拖拽发送) | clawx-vault, clawx-kb | VaultEngine | workspace/imports | 4.1, 4.4 | v0.1 |
| 2.1 | Agent 状态展示 | clawx-runtime, clawx-ffi | Observer | agents (status) | 4.1 | v0.1 |
| 2.1 | Agent 模板库 | clawx-config | — | agents | — | v0.1 |
| 2.1 | Per-Agent 绑定模型 | clawx-llm | ModelRegistry | model_providers, agents | 4.1 | v0.1 |
| **2.2** 数据保险箱 | 自动版本点 | clawx-vault | VaultEngine | version_points, changesets, blobs | 4.4 | v0.1 |
| 2.2 | 变更集记录 | clawx-vault | VaultEngine | changesets | 4.4 | v0.1 |
| 2.2 | Skills 安装版本点 | clawx-vault | VaultEngine | version_points | 4.11 | v0.2 |
| 2.2 | 版本管理界面 | clawx-ffi (SwiftUI) | — | version_points | — | v0.1 |
| 2.2 | 差异预览 | clawx-vault | VaultEngine (diff_version) | changesets | 4.4 | v0.1 |
| 2.2 | 文件级还原 | clawx-vault | VaultEngine (restore_file) | changesets, blobs | 4.4 | v0.1 |
| 2.2 | 任务级回滚 | clawx-vault | VaultEngine (restore_task) | changesets | 4.4 | v0.1 |
| 2.2 | 工作区边界控制 | clawx-vault, clawx-security | VaultEngine, SecurityGate | — | 4.4 | v0.1 |
| 2.2 | 智能清理 | clawx-vault, clawx-daemon | VaultEngine (cleanup) | blobs (ref_count) | 4.4 | v0.1 |
| **2.3** 记忆中心 | Agent 记忆 | clawx-memory | MemoryStore | agent_memories | 4.1 | v0.1 |
| 2.3 | 用户记忆 (全局共享) | clawx-memory | MemoryStore | user_memories | 4.1 | v0.1 |
| 2.3 | 结构化存储 | clawx-memory | — | agent/user_memories | — | v0.1 |
| 2.3 | 智能遗忘 (艾宾浩斯) | clawx-memory | MemoryStore (decay_pass) | freshness 字段 | — | v0.1 |
| 2.3 | 记忆主动提取 | clawx-memory | MemoryStore (smart_inject) | — | 4.1 | v0.1 |
| 2.3 | Memory Flush | clawx-memory | MemoryStore (flush_before_compaction) | — | 4.1 | v0.1 |
| **2.4** 知识库引擎 | 文件夹监控 (FSEvents) | clawx-kb | KnowledgeEngine | kb_sources | — | v0.2 |
| 2.4 | 多格式解析 | clawx-kb, clawx-hal | KnowledgeEngine | kb_files | — | v0.2 |
| 2.4 | 智能分块 | clawx-kb | — | kb_chunks | — | v0.2 |
| 2.4 | 混合检索 (向量+BM25+RRF) | clawx-kb | KnowledgeEngine (search) | kb_chunks | 4.1 | v0.2 |
| 2.4 | 本地 Embedding | clawx-kb, clawx-hal | AcceleratorProvider | embedding_id | — | v0.2 |
| **2.5** 安全执行官 | 分级执行 (T1/T2/T3) | clawx-security | SecurityGate | — | 4.1 | v0.1 (基础), v0.2 (完整) |
| 2.5 | Capability 权限模型 | clawx-security | SecurityGate | — | 4.1 | v0.1 |
| 2.5 | 网络白名单 | clawx-security, clawx-config | — | network-whitelist.toml | — | v0.1 |
| 2.5 | Prompt 注入防御 (3 层) | clawx-security | SecurityGate (prompt_injection_scan) | — | 4.1 | v0.1 |
| 2.5 | DLP 双向扫描 | clawx-security | SecurityGate (dlp_scan) | — | 4.1 | v0.1 |
| 2.5 | 数据脱敏上云 | clawx-security | — | — | 4.1 | v0.1 |
| 2.5 | 高风险操作确认 | clawx-security, clawx-ffi | SecurityGate | — | 4.1 | v0.1 |
| 2.5 | Merkle 审计链 | clawx-security | SecurityGate (audit) | audit_log | — | v0.1 |
| 2.5 | 凭证注入 | clawx-security | CredentialInjector | — | 3.5.1 | v0.1 |
| 2.5 | 污点跟踪 | clawx-security | — (TaintLabel) | — | — | v0.3 |
| 2.5 | WASM 双计量 | clawx-skills | — | — | — | v0.2 |
| **2.6** IM 渠道 | 渠道管理 | clawx-channel | ChannelAdapter | agent_channels | 4.2 | v0.2 |
| 2.6 | Binding Rules 路由 | clawx-gateway | Gateway | binding_rules | 4.2 | v0.2 |
| 2.6 | 会话隔离 | clawx-gateway | Gateway (Lane 队列) | — | 4.2 | v0.2 |
| 2.6 | 主动通知 | clawx-scheduler, clawx-channel | ProactiveScheduler | scheduled_jobs | 4.3 | v0.2 |
| **2.7** Skills 生态 | Skills 本地管理 | clawx-skills | — | skills, agent_skills | — | v0.2 |
| 2.7 | 开源 Skills 接入 | clawx-skills | SkillsStoreClient | skills | — | v0.3 |
| 2.7 | Skills 商店 | clawx-skills | SkillsStoreClient | skills, skill_licenses | 4.11 | v0.5 |
| 2.7 | Skills 商业化 | clawx-skills | SkillsStoreClient (purchase) | skill_licenses | 4.11 | v0.5 |
| 2.7 | Skills 安全卫士 (4 层) | clawx-security, clawx-skills | SecurityGate | audit_log | 4.11 | v1.0 |
| **2.8** 主动式 Agent | 定时任务 (Cron) | clawx-scheduler | ProactiveScheduler | scheduled_jobs | 4.3 | v0.2 |
| 2.8 | 事件驱动 | clawx-scheduler, clawx-eventbus | ProactiveScheduler | scheduled_jobs | 4.3 | v0.2 |
| 2.8 | 上下文感知 | clawx-scheduler, clawx-memory | ProactiveScheduler | scheduled_jobs | 4.3 | v0.2 |
| 2.8 | 对话创建定时任务 (NL-to-Cron) | clawx-scheduler, clawx-llm | ProactiveScheduler | scheduled_jobs | 4.3 | v0.2 |
| 2.8 | 多渠道通知 | clawx-scheduler, clawx-channel | ProactiveScheduler (notify_result) | — | 4.3 | v0.2 |
| 2.8 | 后台可靠执行 | clawx-daemon | Watchdog | — | — | v0.2 |
| 2.8 | 任务管理面板 | clawx-ffi (SwiftUI) | — | scheduled_jobs | — | v0.2 |
| 2.8 | 反馈循环 | clawx-scheduler | ProactiveScheduler (record_feedback) | proactive_feedback | 4.3 | v0.2 |
| **2.9** 产物管理 | 统一文件列表 | clawx-artifact | ArtifactStore (list) | artifacts | — | v0.3 |
| 2.9 | 文件预览 | clawx-artifact | ArtifactStore (generate_preview) | artifacts | — | v0.3 |
| 2.9 | 来源追溯 | clawx-artifact | — | artifacts (agent_id, task_id) | — | v0.3 |
| 2.9 | 导出与分享 | clawx-artifact | ArtifactStore (export, share_via_channel) | artifacts | — | v0.3 |
| 2.9 | 存储管理 | clawx-artifact | ArtifactStore (cleanup) | artifacts | — | v0.3 |
| **2.10** 模型管理 | 多 Provider 支持 | clawx-llm | ModelRegistry | model_providers | — | v0.1 |
| 2.10 | API Key 管理 (Keychain) | clawx-llm | ModelRegistry (rotate_key) | model_providers | — | v0.1 |
| 2.10 | Per-Agent 绑定 | clawx-llm | ModelRegistry (bind_agent_model) | agents, model_providers | — | v0.1 |
| 2.10 | 模型测试 | clawx-llm | ModelRegistry (test_connection) | — | — | v0.1 |
| 2.10 | 用量统计 | clawx-llm | UsageAggregator | usage_stats | — | v0.3 |
| **2.11** 账号体系 | 邮箱/手机登录 | clawx-account | AuthProvider | sessions | 4.5 | v0.3 |
| 2.11 | Google/Apple OAuth | clawx-account | AuthProvider | sessions | 4.5 | v0.3 |
| 2.11 | 会话管理 | clawx-account | AuthProvider | sessions, user_profiles | 4.5 | v0.3 |
| **2.12** 同步备份 | 数据同步 | clawx-sync | SyncEngine | sync_state | 4.6 | v0.5 |
| 2.12 | 全量加密备份 | clawx-sync | SyncEngine (create_backup) | backup_manifests | 4.6 | v0.5 |
| 2.12 | 增量同步 | clawx-sync | SyncEngine (incremental_sync) | sync_state | 4.6 | v0.5 |
| 2.12 | 换机恢复 | clawx-sync | SyncEngine (restore_backup) | backup_manifests | 4.6 | v0.5 |
| **2.13** HireClaw 社区 | Agent 发布 | clawx-community | CommunityClient | — (云端) | — | v1.0 |
| 2.13 | Agent 搜索/下载 | clawx-community | CommunityClient | — (云端) | — | v1.0 |
| 2.13 | Agent 评价 | clawx-community | CommunityClient (rate_agent) | — (云端) | — | v1.0 |
| 2.13 | Agent 注册 (唯一 ID) | clawx-community | CommunityClient (register_agent_id) | — (云端) | — | v1.0 |
| 2.13 | Agent 间对话 | clawx-community | CommunityClient (inter_agent_message) | — (云端) | — | v1.0 |
| 2.13 | Agent 商业化 | clawx-community | CommunityClient | — (云端) | — | v1.0 |
| **2.14** OpenClaw 迁移 | 自动检测 | clawx-migration | MigrationEngine (detect) | — | 4.10 | v0.3 |
| 2.14 | 迁移预览 | clawx-migration | MigrationEngine (plan) | migration_log | 4.10 | v0.3 |
| 2.14 | 执行迁移 | clawx-migration | MigrationEngine (execute) | migration_log | 4.10 | v0.3 |
| 2.14 | 增量迁移 | clawx-migration | MigrationEngine (incremental_sync) | migration_log | 4.10 | v0.3 |
| 2.14 | 回滚 | clawx-migration, clawx-vault | MigrationEngine (rollback) | migration_log | 4.10 | v0.3 |
| **2.15** 系统稳定性 | 进程守护 (< 5s 重启) | clawx-daemon | Watchdog | — | — | v0.1 (基础), v0.3 (完整) |
| 2.15 | 心跳检测 | clawx-daemon | Watchdog (register_module) | — | — | v0.3 |
| 2.15 | 内存泄漏检测 | clawx-daemon | Watchdog | — | — | v0.3 |
| 2.15 | 开机自启动 | clawx-daemon | — (launchd plist) | — | — | v0.1 |
| 2.15 | 系统健康面板 | clawx-daemon, clawx-ffi | Watchdog (health_panel) | — | — | v0.3 |
| **2.16** OTA 更新 | 自动检查更新 | clawx-ota | UpdateEngine (check_update) | update_history | 4.9 | v0.4 |
| 2.16 | 手动检查更新 | clawx-ota, clawx-ffi | UpdateEngine | update_history | 4.9 | v0.4 |
| 2.16 | 增量更新 (Delta) | clawx-ota | UpdateEngine (download) | update_history | 4.9 | v0.4 |
| 2.16 | 签名验证 | clawx-ota | UpdateEngine (verify) | — | 4.9 | v0.4 |
| 2.16 | 安全补丁静默安装 | clawx-ota | UpdateEngine | update_history | 4.9 | v0.4 |
| 2.16 | 更新回滚 | clawx-ota | UpdateEngine (rollback) | update_history | 4.9 | v0.4 |
| **2.17** 物理 Agent | 设备发现 | clawx-physical | DeviceAdapter (discover) | devices | 4.7 | v0.5 |
| 2.17 | 摄像头集成 (RTSP/ONVIF) | clawx-physical | CameraAdapter | devices | 4.7 | v0.5 |
| 2.17 | 智能家居 (MQTT/HomeKit) | clawx-physical | SmartHomeAdapter | devices | 4.7 | v0.5 |
| 2.17 | 对话式控制 | clawx-physical, clawx-runtime | DeviceAdapter (send_command) | — | 4.7 | v0.5 |
| 2.17 | 智能任务联动 | clawx-physical, clawx-scheduler | — | device_automations | 4.7 | v0.5 |
| 2.17 | 设备管理面板 | clawx-physical, clawx-ffi | DeviceAdapter (list_devices) | devices | — | v0.5 |
| **2.18** 移动端随行 | 远程对话 | clawx-mobile-relay | RelayServer (stream_chat) | paired_devices | 4.8 | v1.0 |
| 2.18 | 通知接收 | clawx-mobile-relay | RelayServer (push_notification) | paired_devices | 4.8 | v1.0 |
| 2.18 | 任务管理 | clawx-mobile-relay | RelayServer | — | 4.8 | v1.0 |
| 2.18 | Agent 状态 | clawx-mobile-relay | RelayServer (get_agents_health) | — | 4.8 | v1.0 |
| 2.18 | 安全连接 (WireGuard) | clawx-mobile-relay | RelayServer | paired_devices | 4.8 | v1.0 |
| **2.19** 基础设置 | 国际化 (i18n) | clawx-settings | SettingsStore (get/set_locale) | settings | — | v0.1 |
| 2.19 | 外观设置 (主题) | clawx-settings | SettingsStore (get/set_theme) | settings | — | v0.1 |
| 2.19 | 用户协议 | clawx-ffi (SwiftUI) | — | — | — | v0.1 |
| 2.19 | 反馈入口 | clawx-ffi (SwiftUI) | — | — | — | v0.1 |
| **3.x** 非功能性需求 | 空闲 CPU < 3% | clawx-daemon | Observer (on_metrics) | — | — | v0.1+ |
| 3.x | 空闲内存 < 300MB | clawx-daemon | Watchdog | — | — | v0.1+ |
| 3.x | 冷启动 < 2s | AD-03 懒加载 | — | — | — | v0.1 |
| 3.x | 年可用率 >= 99.9% | clawx-daemon | Watchdog | — | — | v0.3 |
| 3.x | TLS 1.3 | clawx-security | — | — | — | v0.1 |
| 3.x | Skills 签名 (Ed25519) | clawx-skills | — | skills (signature) | — | v0.2 |
| 3.x | 图形化安装向导 | clawx-gui | — | — | — | v0.1 |
| 3.x | macOS 14+ / ARM+Intel | Cargo 双 target | — | — | — | v0.1 |
