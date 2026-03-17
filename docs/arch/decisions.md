# ClawX 架构决策记录 (ADR)

**版本:** 4.0
**日期:** 2026年3月18日

---

## ADR-001: Rust Workspace 分层单体架构

**状态:** 已采纳

**背景:** ClawX 是本地桌面应用，需要高性能、低资源占用、内存安全。

**决策:** 采用 Rust Workspace 组织 21 个 crate，按 Foundation → Core → Domain → Infrastructure → API 分层。

**理由:**
- 本地应用无需微服务拆分开销
- Crate 级模块化已提供充分的隔离和可测试性
- 编译期依赖检查确保层级约束
- 统一发布版本，避免版本不一致问题

**后果:** 所有模块编译为单一二进制，部署简单；但编译时间随 crate 增加而增长。

---

## ADR-002: SQLite 作为主数据库

**状态:** 已采纳

**背景:** 需要持久化存储 Agent 配置、对话、记忆等结构化数据。

**决策:** 使用 SQLite (通过 sqlx) 作为主数据库，文件存储在 `~/.clawx/db/clawx.db`。

**替代方案:**
- PostgreSQL：功能强大但需要独立进程，违背本地零运维原则
- sled/rocksdb：KV 存储，缺少 SQL 查询灵活性

**理由:**
- 嵌入式零运维，完美契合本地优先
- sqlx 提供编译期 SQL 检查
- 单文件数据库便于备份和迁移
- 性能满足单用户场景

**后果:** 不支持高并发写入，但单用户场景无此需求。

---

## ADR-003: Qdrant Embedded + Tantivy 混合检索

**状态:** 已采纳

**背景:** 知识库需要同时支持语义检索和关键词检索。

**决策:**
- 向量检索：Qdrant embedded 模式（嵌入进程内，无需独立服务）
- 全文检索：Tantivy (Rust 原生 BM25 引擎)
- 融合策略：RRF (Reciprocal Rank Fusion)

**替代方案:**
- 纯向量检索：语义理解好但关键词匹配差
- 纯 BM25：关键词精确但缺乏语义理解
- Elasticsearch：需要 JVM，资源占用过大

**理由:**
- 混合检索在中英混合场景下效果显著优于单一方案
- 嵌入式模式无需额外进程管理
- Tantivy 是 Rust 原生，无 FFI 开销

---

## ADR-004: EventBus 模块间通信（分阶段启用）

**状态:** 已采纳（分阶段实施）

**背景:** 19 个 domain crate 之间需要松耦合通信。

**决策:** 保留 EventBus 架构位（clawx-eventbus crate），但分阶段实施：
- **v0.1**：模块间使用 Trait 直接调用，简单直接
- **v0.2+**：当 1:N 广播场景频繁出现后，启用 EventBus 实现替换直调

**替代方案:**
- 全程 Trait 直调：简单但 1:N 场景需要手动扇出，容易遗漏
- IPC (Unix Socket / gRPC)：本地进程内不必要的序列化开销

**理由:**
- v0.1 模块数量有限，Trait 直调足够且调试方便
- v0.2 引入 Skills、Scheduler、Channel 后，1:N 事件广播需求增多（如"文件删除"需同时通知 Vault、Security、Artifact）
- 保留 crate 和接口定义，确保后续启用时不需要大规模重构

**后果:** v0.1 阶段调试简单；v0.2 切换时需要逐步替换直调为事件发布。

---

## ADR-005: 三级安全执行模型

**状态:** 已采纳

**背景:** Agent 执行的操作风险等级不同，需要差异化安全策略。

**决策:** T1 (WASM 沙箱) → T2 (受限子进程) → T3 (原生逐次确认) 三级模型。

**理由:**
- 并非所有操作都适合 WASM（如 Shell 命令、文件 I/O）
- 分级模型在安全性和功能可用性之间取得平衡
- 默认最严格，逐步放宽

---

## ADR-006: SwiftUI + FFI 而非 Web UI

**状态:** 已采纳

**背景:** GUI 技术选型。

**决策:** macOS 原生 SwiftUI，通过 swift-bridge/uniffi 与 Rust Core 通信。

**替代方案:**
- Tauri (Web UI)：跨平台但体验不如原生
- Electron：资源占用大，与"轻量"目标矛盾
- egui (Rust native)：生态不成熟，macOS 原生集成差

**理由:**
- macOS 原生体验（动画、Touch Bar、通知中心、Spotlight 集成）
- SwiftUI 是 Apple 一等公民框架
- FFI 比 IPC 延迟更低

**后果:** 仅支持 macOS，但这是 v1 的目标平台。

---

## ADR-007: 双进程模型 + launchd 守护

**状态:** 已采纳

**背景:** 需要支持 GUI 关闭后后台任务继续运行，且进程崩溃后自动恢复。

**决策:**
- `clawx-service`：后台进程（无 UI），由 macOS launchd 守护
- `ClawX.app`：GUI 进程，通过 FFI/HTTP 与 service 通信
- 进程守护由 **macOS launchd** 提供（KeepAlive + RunAtLoad），不自研守护进程
- `clawx-daemon` 模块仅负责：生成 plist、进程内健康自检、崩溃恢复状态

**替代方案:**
- 自研 watchdog 进程：增加复杂度，且不如 launchd 可靠（launchd 是 PID 1 子系统）
- 单进程模型：GUI 关闭则后台任务中断

**理由:**
- launchd 是 macOS 最可靠的进程管理器，开机自启 + 崩溃重启 < 5s
- 定时任务、IM 渠道监听不依赖 GUI 打开
- GUI 可随时关闭/重启不影响后台逻辑

---

## ADR-008: TOML 配置格式

**状态:** 已采纳

**背景:** 应用配置格式选型。

**决策:** TOML (`~/.clawx/config.toml`)

**替代方案:** YAML、JSON

**理由:**
- Rust 生态标准（Cargo.toml 同格式）
- 人类可读可编辑
- 类型明确（字符串/整数/布尔不歧义）
- 优秀的注释支持

---

## ADR-009: 本地 Embedding 模型

**状态:** 已采纳

**背景:** 知识库向量化需要 Embedding 模型。

**决策:** 采用本地 Embedding 模型，候选：
- 文本：`nomic-embed-text` (轻量) 或 `bge-m3` (多语言)
- 多模态：`CLIP ViT-B/32` (文本+图像)

**选型标准:**
- 模型体积 < 500MB
- 推理延迟 < 50ms/chunk
- 支持 Apple Silicon 加速 (CoreML/Metal)

**理由:** 本地优先原则，Embedding 不应依赖云端 API。

---

## ADR-010: 审计日志链式校验

**状态:** 已采纳

**背景:** 审计日志不可篡改是安全合规要求。

**决策:** 每条日志包含前一条的 SHA-256 哈希，形成哈希链。追加写入 JSONL 格式。

**理由:**
- 任何中间篡改都会破坏哈希链
- JSONL 格式支持高效追加和流式读取
- 按日期分文件，便于归档和清理

---

## ADR-011: macOS Keychain 密钥存储

**状态:** 已采纳

**背景:** API Key、Token 等敏感凭证的安全存储。

**决策:** 使用 macOS Keychain 存储所有敏感凭证，运行时通过 `secret:inject:{name}` 权限按需注入。

**理由:**
- macOS 原生安全存储，硬件级加密
- 避免明文存储在配置文件中
- 统一的密钥生命周期管理

---

## ADR-012: 移动端通过 Cloud Relay 通信

**状态:** 已采纳

**背景:** 移动端（iOS）需要远程访问 Mac 主机上的 Agent。最初考虑 P2P 隧道（Tailscale/WireGuard），但对普通用户门槛过高。

**决策:** 通过云端 Relay 转发服务实现移动端与 Mac 主机的通信。

**替代方案:**
- Tailscale / WireGuard P2P 隧道：安全但需要用户自行配置，门槛高
- 直接公网暴露：严重安全风险
- iCloud CloudKit：受限于 Apple 生态，功能有限

**设计要点:**
- Mac 通过 WSS 长连接注册到 Relay
- iOS 通过 HTTPS 连接 Relay，发送指令、接收结果
- Mac 和 iOS 之间 X25519 协商 E2E 加密，Relay 仅转发密文，不可解密
- Relay 集成 APNs 推送代理，支持离线消息缓存（TTL 7 天）
- 依赖账号体系（同一账号下设备互相发现）

**理由:**
- 用户零配置，登录账号即可使用
- E2E 加密保障数据主权（Relay 不可解密）
- 云服务仅做路由，不存储用户数据，合规风险低

**后果:** 需要部署和运维 Cloud Relay 后端服务；依赖 v0.3+ 账号体系。

---

## ADR-013: 保留 HAL 硬件抽象层

**状态:** 已采纳

**背景:** V1 仅支持 macOS，是否需要硬件抽象层。

**决策:** 保留 `clawx-hal` 作为 macOS 系统 API 的统一抽象层。

**替代方案:**
- 各模块直接调用 macOS API：短期简单但后续难以统一管理和测试

**理由:**
- 统一封装 macOS 平台相关 API（FSEvents、Keychain、Notification、pf、系统监控）
- 便于单元测试（可 mock HAL Trait）
- 为未来可能的 Linux 支持预留抽象接口
- 物理 Agent 接入（摄像头、智能家居）也需要统一的设备抽象

---

## ADR-014: 四层记忆架构 (Working + Short-Term + Long-Term + Reflection)

**状态:** 已采纳 (v2.0 更新，原三层升级为四层)

**背景:** 调研 MemGPT/Letta、Mem0、Zep/Graphiti、CrewAI、Generative Agents 等业界方案后发现：
- Mem0/Zep/CrewAI 均将实体记忆作为核心组件，ClawX 原设计缺少实体追踪
- Generative Agents (Park et al.) 的周期性反思机制是该论文最重要贡献之一
- MAGMA 的四图架构在推理准确率上提升 45.5%

**决策:** 升级为四层记忆架构：
- **Working Memory**：上下文窗口管理 + 递归摘要压缩 (参考 MemGPT)
- **Short-Term Memory**：Session 级缓冲 + 晋升评估
- **Long-Term Memory**：持久化存储，含三个子类型：
  - Agent Memory (私有) + User Memory (共享) + **Entity Memory (实体与关系)**
- **Reflection** [v0.2]：周期性高阶反思 + 失败经验学习 (参考 Generative Agents + Reflexion)

**替代方案:**
- 仅三层 (无实体/反思)：缺少结构化关系追踪和认知升华能力
- MAGMA 四图架构：效果最佳但复杂度极高，不适合嵌入式本地应用
- Zep/Graphiti 时序知识图谱：需要图数据库，违背零运维原则

**理由:**
- Entity Memory 用 SQLite 关系表实现，保持嵌入式架构，覆盖 80% 实体关系场景
- Reflection 在 v0.2 引入，此时已有足够记忆量支撑有效反思
- 四层架构比三层更完整，但不引入图数据库，避免过度工程化

**详细设计:** 见 [memory-architecture.md](./memory-architecture.md)

---

## ADR-015: 记忆提取 LLM 辅助 + Agent 自主双轨制

**状态:** 已采纳 (v2.0 更新，新增 Agent 自主管理)

**背景:** 调研发现 MemGPT/Letta 的核心创新是让 Agent 通过函数调用自行管理记忆 (self-directed memory)，效果最佳但每次操作消耗 LLM 推理。

**决策:** 双轨制：
- v0.1：系统自动提取为主 (LLM 辅助隐式提取 + 信号词触发)
- v0.2：新增 MemGPT 风格 Agent Memory Tools (memory_save/search/update/entity_lookup)

**替代方案:**
- 纯规则引擎：无法理解语义，漏提严重
- 纯 MemGPT 自主管理：每次操作消耗 LLM 推理，v0.1 开销过大
- 每轮都调 LLM：Token 开销不可控

**理由:**
- 系统自动提取保底不遗漏
- Agent 自主管理提升主动性和精准度
- Agent 通过 Tool 主动存储的记忆优先级高于系统隐式提取

---

## ADR-016: SQLite SoT + 三路混合检索 (向量 + BM25 + 实体)

**状态:** 已采纳 (v2.0 更新，从双路升级为三路)

**背景:** 业界共识 dense + sparse 混合检索显著优于单一方案。ClawX 已有 Tantivy 引擎 (知识库模块可复用)。新增的 Entity Memory 提供结构化关系检索能力。

**决策:** 三路并行检索 + RRF 融合：
1. Qdrant 向量语义检索
2. Tantivy BM25 关键词检索 (复用已有基础设施)
3. Entity 实体关系检索 (SQLite 关系查询)
4. RRF (Reciprocal Rank Fusion) 融合排序

SQLite 为 Source of Truth，Qdrant/Tantivy 均为可重建索引。

**替代方案:**
- 仅向量检索：关键词精确匹配差 (如人名"张三")
- 仅 BM25：缺乏语义理解
- 全文 Graphiti 图检索：需要图数据库

**理由:**
- 三路检索在中英混合场景下 Top-5 hit rate 提升 >= 5 个百分点
- Tantivy 复用知识库已有引擎，零额外基础设施成本
- 实体关系检索解决 "张三在 Project X 做什么" 类查询

---

## ADR-017: 12 层纵深防御安全架构

**状态:** 已采纳

**背景:** 调研 OpenClaw 生态中的安全实践后发现，ClawX v3.0 的 6 层安全架构存在明显短板：
- IronClaw 实施了 13 层安全管道 + WASM 组件模型沙箱 + Aho-Corasick 泄漏检测 + 宿主边界凭证注入
- OpenFang 实施了 16 层纵深防御 + WASM 双计量 + 密钥零化 + 信息流污点追踪 + SSRF 防护
- OpenClaw 曾暴露 CVE-2026-25253，其共享内存 Node 进程架构的安全性受到广泛质疑

**决策:** 将 ClawX 安全架构从 6 层升级为 12 层纵深防御体系。新增 6 层安全能力：

| 新增层 | 参考来源 | 说明 |
|--------|---------|------|
| L3 宿主边界凭证注入 | IronClaw | 密钥永不进入 WASM 沙箱 |
| L6 SSRF 防护 | OpenFang | 拦截私有 IP/云元数据/DNS 重绑定 |
| L7 路径穿越防护 | OpenFang | 规范化 + 符号链接检查 |
| L8 密钥零化 | OpenFang | `Zeroizing<String>` 使用后擦除 |
| L9 循环守卫 | OpenFang | 检测调用链乒乓模式 |
| L10 Ed25519 签名 | OpenFang | Skill 包防供应链攻击 |
| L11 GCRA 速率限制 | OpenFang | 多维度精确限速 |

原有层增强：
- L2 WASM 沙箱升级为双计量（燃料 + 纪元中断），参考 OpenFang
- L5 DLP 增加 Aho-Corasick 多模式匹配优化，参考 IronClaw LeakDetector
- L12 审计日志增加哈希链完整性校验

**不采纳的 OpenFang 能力：**
- **信息流污点追踪**：实现复杂度极高，需要在整个类型系统中传播标签。v1 阶段用 DLP 扫描 + 宿主边界凭证注入已能覆盖主要风险场景。列入长期观察
- **Merkle 树审计**：SHA-256 哈希链在单机场景下已足够。Merkle 树更适合分布式多节点审计场景
- **OFP 互认证**：ClawX v1 不涉及 P2P Agent 网络，暂不需要

**理由:**
- 12 层在安全覆盖面上接近 IronClaw (13) 和 OpenFang (16)，同时避免过度工程化
- 每层独立可测试，单层失效不导致全面突破
- 新增的安全能力优先选择 Rust 生态成熟 crate（zeroize, aho-corasick），降低自研风险
- 分阶段实施，v0.1 先落地最关键的 7 层，v0.2 完成全部 12 层

---

## ADR-018: 智能模型路由

**状态:** 已采纳

**背景:** 调研发现 IronClaw 的智能模型路由（13 维度复杂度评分）和 OpenFang 的成本追踪 + 自动回退机制能显著降低 LLM 使用成本，同时维持用户体验。

**决策:** 在 clawx-llm 中实现智能模型路由，支持三种模式：

1. **固定模型**（默认）：用户在 Per-Agent 配置中指定固定模型
2. **智能路由**：按请求复杂度自动选择 Flash/Standard/Pro 三个层级的模型
3. **级联模式**：先用低成本模型尝试，若置信度不足自动升级

**替代方案:**
- 仅支持固定模型绑定：简单但用户无法自动优化成本
- 每次都用最强模型：效果好但成本高，不适合高频场景

**理由:**
- 参考 IronClaw 实践，智能路由可降低 50-70% LLM 成本
- 级联模式在保证质量的前提下进一步优化成本
- 不改变现有 Per-Agent 绑定模型的默认行为，智能路由为可选增强
- 配合预算追踪功能，帮助用户控制 Token 消耗

---

## ADR-019: 宿主边界凭证注入

**状态:** 已采纳

**背景:** IronClaw 的凭证注入设计是其安全架构中最独特的模式——WASM 沙箱内的 Tool 代码永远接触不到密钥明文。Tool 通过占位符引用密钥，宿主在 HTTP 调用的最后一步替换为真实值。

**决策:** 采纳 IronClaw 的宿主边界凭证注入模式：

1. WASM Tool 通过占位符语法引用密钥：`{SECRET_NAME}`
2. Tool 构造的 HTTP 请求中包含占位符
3. 宿主侧接收请求后，执行域名白名单检查
4. 从 Keychain 读取密钥（Zeroizing 包装）
5. 替换占位符，执行真实 HTTP 请求
6. 密钥自动零化
7. DLP 扫描响应后返回给沙箱

**替代方案:**
- 将密钥注入 WASM 环境变量：密钥在沙箱内可见，恶意代码可提取
- 由 Tool 自行从 API 获取密钥：需要暴露密钥 API，攻击面大

**理由:**
- 密钥永不进入沙箱，即使 WASM Tool 被完全攻破也无法获取密钥
- 宿主侧可同时执行白名单检查、DLP 扫描、速率限制
- 与密钥零化 (Zeroizing) 结合，密钥在内存中的暴露窗口最小化
- `secret_exists(name)` 只读检查接口让 Tool 可以检测密钥是否已配置，而不暴露值

---

## ADR-020: MCP 协议支持

**状态:** 已采纳

**背景:** Model Context Protocol (MCP) 是 LLM 生态中正在形成的工具集成标准协议。NanoBot 以 MCP 为核心架构，OpenFang 支持 25 个 MCP 模板，多个竞品项目均已支持。

**决策:** 在 clawx-skills 模块中支持 MCP 协议，允许 ClawX 作为 MCP 客户端连接外部 MCP 工具服务器。

**替代方案:**
- 仅支持 WASM Skills：限制了工具生态的扩展性
- 自定义 RPC 协议：增加开发者学习成本

**理由:**
- MCP 是行业趋势，支持 MCP 可直接复用大量已有的 MCP 工具服务器
- 降低第三方开发者为 ClawX 开发工具的门槛
- 不替代 WASM Skills，而是作为互补的工具接入方式
- v0.2 阶段实现，与 Skills 本地管理同期交付

---

## ADR-021: 轻量级实体记忆 (SQLite 关系表，非图数据库)

**状态:** 已采纳

**背景:** 调研发现实体与关系追踪是当前 Agent 记忆系统的重要趋势：
- Zep 使用时序知识图谱 (Graphiti)，DMR 基准准确率 94.8%
- Mem0 的图记忆 ($24M 融资) 通过 vector + graph + KV 混合架构实现
- MAGMA 的四图架构 (语义/时序/因果/实体) 推理准确率提升 45.5%
- CrewAI 将 Entity Memory 作为四类记忆之一

**决策:** v1 使用 SQLite `entities` + `entity_relations` 表实现实体记忆，不引入 Neo4j 等图数据库。实体提取复用记忆提取的 LLM 调用（同一次 Prompt 同时提取记忆和实体）。

**替代方案:**
- Graphiti/Neo4j 时序知识图谱：效果最佳但需要独立图数据库进程，违背嵌入式本地优先
- 纯向量 (无实体)：AutoGPT 方案，"张三在 Project X 做什么" 类查询效果差
- 完整 MAGMA 四图：过度工程化，v1 单用户场景不需要因果图

**理由:**
- SQLite 关系表覆盖 80% 实体关系场景 (一跳查询 + 简单多跳)
- 保持零运维嵌入式架构，不增加运维复杂度
- 实体提取与记忆提取同步进行，无额外 LLM 调用开销
- v2 可在需要时升级为 SQLite + 轻量图索引

---

## ADR-022: 记忆冲突解决采用 LLM 辅助分类 + 版本化保留

**状态:** 已采纳

**背景:** 调研发现 Last-Write-Wins 是业界最弱的冲突解决方案：
- Mem0 的 active curation 通过 LLM 分类冲突类型并智能处理，标记旧值为 inactive
- Zep 的时序版本化维护事实的时间有效性
- 大多数系统的冲突解决仍是开放问题，但至少需要版本化保留

**决策:** 当新记忆与已有记忆语义相似度在 0.7-0.92 区间时，调用 LLM 分类冲突类型：
- DUPLICATE：去重，保留已有
- UPDATE：更新已有记忆，旧版本存入 `memory_versions` 表
- CONTRADICTION：按重要性处理（< 8 自动替代，>= 8 请求用户确认）
- RELATED：两条均保留
- UNRELATED：存储为新记忆

所有被替代的旧版本保留在 `memory_versions` 表，支持回溯和审计。

**替代方案:**
- Last-Write-Wins：最简单但容易丢失正确信息
- 全部保留 (append-only)：不解决矛盾，检索时噪声大
- 用户手动解决所有冲突：体验差，频繁打断

**理由:**
- LLM 分类比固定规则更准确，能区分"更新"和"矛盾"
- 版本化保留支持审计和回溯
- 高重要性冲突由用户裁决，避免自动化误判造成信息损失
