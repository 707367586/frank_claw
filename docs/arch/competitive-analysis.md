# 竞品深度分析：OpenClaw / IronClaw / ZeroClaw / OpenFang

**日期:** 2026年3月17日
**目的:** 提炼各竞品的架构精华，输出 ClawX 架构改进项

---

## 1. 竞品全景概览

| 维度 | OpenClaw | ZeroClaw | IronClaw | OpenFang | **ClawX** |
|------|----------|----------|----------|----------|-----------|
| **语言** | TypeScript/Node.js | Rust | Rust | Rust (14 crates) | Rust + SwiftUI |
| **Stars** | 247K+ | 20K+ | - | 7K+ (2周) | - |
| **Binary 大小** | ~500 MB (npm) | 3.4 MB | - | ~32 MB | 目标 < 30 MB |
| **内存** | > 1 GB | < 5 MB (峰值 < 8 MB) | 中等 | - | 目标 < 50 MB |
| **启动** | > 500 ms | < 10 ms | 中等 | - | 目标 < 100 ms |
| **渠道** | 23+ | 17+ | 5+ | 40+ | 初期 3-5 |
| **安全模型** | 弱（多次 CVE） | 工作区隔离 + 白名单 | 4 层纵深防御 | 16 个独立安全系统 | 借鉴 IronClaw + OpenFang |
| **记忆** | 文件优先 (Markdown) | SQLite + FTS5 | PostgreSQL/libSQL | SQLite + 向量 + 压缩 | 三层记忆 + 混合检索 |
| **Skills 生态** | ClawHub 13,700+ | 编译时扩展 | WASM 动态工具 | Hands 自治包 | Skills 商店 + MCP |
| **创始人** | Peter Steinberger | 社区 | Illia Polosukhin (Transformer 论文作者) | RightNow AI | Frank |

---

## 2. 各竞品核心架构深度分析

### 2.1 OpenClaw — 渠道之王

**值得借鉴：**

| 模式 | 描述 | ClawX 采纳建议 |
|------|------|---------------|
| **Gateway + Lane 队列** | WebSocket 网关 + 每会话 Lane 序列化执行，防止竞态 | ✅ 采纳。Gateway 组件已规划，补充 Lane 队列模式 |
| **Binding Rules 路由** | 4 级确定性路由：Peer > Guild > Account > Channel 全局 | ✅ 采纳。比我们简单的 channel→agent 映射更灵活 |
| **8 个生命周期 Hook** | before_model_resolve, before_prompt_build, before/after_tool_call 等 | ✅ 采纳。Agent Loop 需要 Hook 扩展点 |
| **混合检索 + MMR 去重 + 时间衰减** | 向量 70% + BM25 30% + MMR λ=0.7 + 半衰期 30 天 | ✅ 采纳。我们的知识库引擎可直接使用此参数 |
| **Memory Flush** | 压缩上下文前触发静默回合，让 LLM 主动持久化关键记忆 | ✅ 采纳。记忆中心增加此机制 |
| **文件优先记忆** | Markdown 文件作为记忆源，透明可检查可版本控制 | ⚠️ 部分采纳。作为可选的 Memory Trait 实现 |

**要避免的教训：**

| 问题 | 描述 | ClawX 对策 |
|------|------|-----------|
| **安全灾难** | CVE-2026-25253 一键 RCE；ClawHub 341 个恶意 Skills；4 万暴露实例 | 安全优先设计，不走"先发布后安全"路线 |
| **过度自治** | Agent 陷入工具循环，消耗数百美元 API 费用 | 强制迭代上限 + Token 预算 + 循环检测 |
| **单线程 Lane** | 慢工具调用阻塞整个会话管线 | 工具执行异步化，超时中断 |
| **配置复杂度** | 多 Agent 路由 + 认证 + 沙箱策略的配置学习曲线陡峭 | GUI 向导 + 合理默认值 |

---

### 2.2 ZeroClaw — 性能之王

**值得借鉴：**

| 模式 | 描述 | ClawX 采纳建议 |
|------|------|---------------|
| **5 个核心 Trait** | Provider / Channel / Tool / Memory / Tunnel — 一切可插拔 | ✅ 已在架构中采纳，我们定义了更多 Trait |
| **编译时 Trait 分发** | 不用 dyn，用泛型静态分发，零 vtable 开销 | ⚠️ 部分采纳。热路径用泛型，需要动态加载的场景用 dyn |
| **ChannelMessage 归一化** | 所有平台消息归一化为统一内部类型 → 单一 Agent Loop | ✅ 采纳。channel crate 的 `IncomingMessage` 已对齐此设计 |
| **嵌入式 SQLite + FTS5** | 无需外部向量数据库也能做混合检索 | ⚠️ 考虑。我们用 Qdrant，但可作为降级后备 |
| **工作区隔离** | Agent 只能访问工作区目录，路径遍历/符号链接/null byte 全部阻断 | ✅ 采纳。security crate 补充路径净化 |
| **命令白名单** | 只有显式允许的命令（git, npm, cargo）可执行 | ✅ 采纳。T2 子进程模式增加命令白名单 |
| **Config 驱动组合** | TOML 配置选择 Trait 实现，无需改代码 | ✅ 已在 clawx-config 中规划 |
| **单二进制分发** | 3.4 MB 静态链接，跨 ARM/x86/RISC-V | ✅ 目标。release profile 已配置 LTO + strip |

---

### 2.3 IronClaw — 安全之王

**值得借鉴：**

| 模式 | 描述 | ClawX 采纳建议 |
|------|------|---------------|
| **🔑 宿主边界凭证注入** | LLM 和工具代码**永远看不到**密钥。密钥只在运行时宿主层注入 HTTP 请求 | ✅✅ **必须采纳**。彻底消除 Prompt 注入窃取凭证的攻击面 |
| **seL4 式 Capability 模型** | 零访问默认 + 显式能力授予（HTTP、secrets、tool invoke） | ✅ 采纳。替换我们简单的权限列表 |
| **双向泄漏扫描** | 出站扫描请求体 + 入站扫描响应，检测凭证反射 | ✅ 采纳。我们目前只有出站扫描 |
| **WIT 标准化工具接口** | WebAssembly Interface Types 定义工具合约 | ✅ 采纳。WASM 工具用 WIT 接口而非自定义 JSON |
| **动态工具生成** | 用户自然语言描述需求 → LLM 生成 WASM 工具 | ⏳ v0.3+ 考虑。前期手动编写 |
| **双后端持久化** | PostgreSQL + pgvector 或 libSQL/Turso | ⚠️ 参考。我们用 SQLite + Qdrant |
| **MCP 原生客户端** | 内置 MCP 客户端连接外部工具服务器 | ✅ 已规划在 Skills 协议兼容层 |
| **Identity Files** | 一致的 Agent 人格跨会话保持 | ✅ 与我们的 AgentConfig.system_prompt 对齐 |
| **10+ 可扩展 Trait** | Database / Channel / Tool / LlmProvider / EmbeddingProvider / NetworkPolicyDecider / Hook / Observer / Tunnel / SuccessEvaluator | ✅ 参考。补充 Observer 和 Hook trait |

---

### 2.4 OpenFang — 架构之王

**值得借鉴：**

| 模式 | 描述 | ClawX 采纳建议 |
|------|------|---------------|
| **🏗️ Kernel 架构** | openfang-kernel 负责编排、RBAC、预算跟踪、调度 — 分离于 runtime | ✅ 采纳。将 runtime 拆分为 kernel（调度/策略）+ executor（执行） |
| **🤖 Hands 概念** | 自治 Agent 包：TOML 清单 + 系统提示 + 操作手册 + 技能知识 + 仪表盘 | ✅✅ **强烈采纳**。这是"主动式 Agent"的最佳实现模式 |
| **WASM 双重计量** | Fuel 计量 + Epoch 中断 + 看门狗线程杀死失控代码 | ✅ 采纳。替换我们简单的超时机制 |
| **🔒 Merkle 审计链** | 每个操作 hash 链接到前一个，篡改一条链全断 | ✅✅ **必须采纳**。替换我们简单的 HMAC 审计日志 |
| **Ed25519 清单签名** | Agent 配置签名编译入二进制 | ✅ 采纳。扩展到 Skill 包签名验证 |
| **Taint Tracking** | 标签从源头到汇聚传播，追踪秘密流向 | ✅ 采纳。DLP 从模式匹配升级为污点跟踪 |
| **SHA256 循环检测 + 熔断** | 检测 tool-call 乒乓模式并熔断 | ✅ 采纳。Agent Loop 增加循环检测 |
| **14 crate 分层架构** | kernel / runtime / api / channels / memory / types / skills / hands / extensions / wire / cli / desktop | ✅ 参考。我们的 19 crate 结构与此对齐 |
| **智能记忆注入** | 避免不必要的 memory_recall 循环 | ✅ 采纳。记忆提取策略优化 |
| **JSONL 会话镜像** | 会话数据同步写入 JSONL 用于调试和恢复 | ✅ 采纳。便于问题排查 |

---

## 3. 架构改进项清单（按优先级排序）

### CRITICAL — 必须在 v0.1 中实现

| # | 改进项 | 来源 | 影响模块 |
|---|--------|------|---------|
| **C1** | **宿主边界凭证注入**：密钥永不进入 LLM 上下文或工具代码 | IronClaw | clawx-security, clawx-llm |
| **C2** | **Merkle hash-chain 审计日志**：每条记录链接前一条，不可篡改 | OpenFang | clawx-security |
| **C3** | **Agent Loop 循环检测 + 熔断**：SHA256 去重 + 强制迭代/Token 上限 | OpenFang + OpenClaw 教训 | clawx-runtime |
| **C4** | **双向泄漏扫描**：出站请求 + 入站响应均扫描凭证 | IronClaw | clawx-security |

### HIGH — v0.1-v0.2 实现

| # | 改进项 | 来源 | 影响模块 |
|---|--------|------|---------|
| **H1** | **Capability-based 权限模型**：零访问默认 + 显式能力授予 | IronClaw (seL4) | clawx-security |
| **H2** | **WASM 双重计量**：Fuel 计量 + Epoch 中断 + 看门狗线程 | OpenFang | clawx-skills |
| **H3** | **Gateway Lane 队列**：每会话序列化执行，防止竞态 | OpenClaw | clawx-gateway |
| **H4** | **8 个生命周期 Hook 点**：Agent Loop 各阶段可注入自定义逻辑 | OpenClaw | clawx-runtime |
| **H5** | **Binding Rules 4 级路由**：Peer > Guild > Account > Channel | OpenClaw | clawx-gateway |
| **H6** | **工作区隔离 + 命令白名单 + 路径净化** | ZeroClaw | clawx-security |
| **H7** | **Hands 自治包模式**：TOML 清单 + 操作手册 + 定时调度 | OpenFang | clawx-scheduler, clawx-skills |
| **H8** | **Memory Flush**：上下文压缩前触发静默回合持久化记忆 | OpenClaw | clawx-memory |

### MEDIUM — v0.2-v0.3 实现

| # | 改进项 | 来源 | 影响模块 |
|---|--------|------|---------|
| **M1** | **Taint Tracking**：污点标签从源到汇聚传播 | OpenFang | clawx-security |
| **M2** | **WIT 标准化工具接口** | IronClaw | clawx-skills |
| **M3** | **MMR 去重 + 时间衰减检索** | OpenClaw | clawx-kb |
| **M4** | **JSONL 会话镜像**用于调试和恢复 | OpenFang | clawx-memory |
| **M5** | **Observer Trait**：可插拔的系统观测点 | IronClaw | clawx-types |
| **M6** | **SQLite FTS5 降级方案**：无 Qdrant 时仍可混合检索 | ZeroClaw | clawx-kb |

---

## 4. 安全模型对比与 ClawX 目标架构

```
                    OpenClaw        ZeroClaw        IronClaw        OpenFang        ClawX (目标)
                    ────────        ────────        ────────        ────────        ────────────
Layer 1: 语言安全    ❌ JS           ✅ Rust          ✅ Rust          ✅ Rust          ✅ Rust
Layer 2: 工具沙箱    ❌ 无            ⚠️ 工作区隔离     ✅ WASM+Cap      ✅ WASM 双计量   ✅ WASM+Cap+双计量
Layer 3: 凭证保护    ⚠️ 文件存储      ⚠️ 加密存储       ✅ 宿主边界注入   ⚠️ 加密存储       ✅ 宿主边界注入
Layer 4: 泄漏检测    ❌ 无            ❌ 无            ✅ 双向扫描       ✅ 污点跟踪       ✅ 双向扫描+污点跟踪
Layer 5: 审计链      ⚠️ 基础日志      ⚠️ 基础日志       ✅ 追加审计       ✅ Merkle 链      ✅ Merkle 链
Layer 6: 注入防御    ❌ 无            ❌ 无            ✅ 结构隔离       ⚠️ 扫描器         ✅ 结构隔离+扫描
Layer 7: 循环防护    ⚠️ 可选          ❌ 无            ❌ 无            ✅ SHA256 熔断    ✅ SHA256 熔断
Layer 8: 网络控制    ❌ 无            ⚠️ 白名单        ✅ 策略代理       ⚠️ SSRF 防护     ✅ 策略代理+白名单
```

**ClawX 安全目标**：融合 IronClaw 的凭证注入和能力模型 + OpenFang 的 Merkle 审计和双计量沙箱，达到 8 层纵深防御。

---

## 5. 关键启示总结

### 最重要的 3 个借鉴

1. **IronClaw 的凭证注入模式** — 密钥永远不进 LLM 上下文。这不是优化，是安全基石。
2. **OpenFang 的 Hands 概念** — 自治 Agent 包 = TOML 清单 + 多阶段操作手册 + 定时调度。这是"主动式 Agent"的最佳实现。
3. **ZeroClaw 的 Trait 驱动设计** — 5 个核心 Trait 让一切可插拔，编译时保证类型安全。

### 最重要的 3 个教训

1. **OpenClaw 的安全灾难** — "先发布后安全"导致 RCE 漏洞、恶意 Skills、4 万暴露实例。安全必须从第一天设计。
2. **OpenClaw 的过度自治** — Agent 陷入工具循环烧钱。必须有强制上限和循环检测。
3. **ZeroClaw 的编译时扩展 vs 运行时插件** — 编译时安全但牺牲热插拔。ClawX 需要在 WASM 运行时加载和 Trait 编译时分发之间取平衡。
