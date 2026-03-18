# ClawX 架构决策记录 (ADR)

**日期:** 2026-03-18 | **对应架构:** v4.1

---

## ADR-001: Rust Workspace 分层单体

**决策:** 使用 Rust Workspace 的分层单体，而不是微服务。

**理由:** 本地应用需要低运维、低进程复杂度，同时保留 crate 级边界。

---

## ADR-002: SQLite 主数据库

**决策:** SQLite (`sqlx`) 作为主数据库，存储在 `~/.clawx/db/clawx.db`。

**理由:** 嵌入式零运维、易备份、适合本地优先架构。

---

## ADR-003: service-owned control plane

**决策:** 生产路径上只有 `clawx-service` 持有 runtime；GUI / CLI 统一走本地控制平面。

**理由:** 避免 GUI 与后台各持一份运行时状态。

---

## ADR-004: 共享 `clawx-controlplane-client`

**决策:** `clawx-ffi` 与 `clawx-cli` 共用 `clawx-controlplane-client`，不直接依赖 runtime。

**理由:** 把"单入口"做成硬边界，而不是约定。

---

## ADR-005: launchd 为唯一守护者

**决策:** 使用 macOS `launchd` 守护 `clawx-service`，不再单独设计 `clawx-daemon` crate / 进程。健康自检功能内置于 `clawx-service`。

**理由:** 避免重复的生命周期抽象。

---

## ADR-006: v0.1 先做本地闭环

**决策:** v0.1 只交付工作台、对话、两层记忆、知识库、Vault、安全基线和本地控制平面。

**理由:** 首版必须在无账号、无渠道、无 Skills 的情况下独立成立。

---

## ADR-007: EventBus 延迟到 v0.2

**决策:** v0.1 以 Trait 直调为主；v0.2 才引入 `clawx-eventbus`。

**理由:** 首版模块少，不值得为未来广播需求提前铺大抽象。

---

## ADR-008: TOML 配置

**决策:** 使用 TOML 作为本地配置格式。

**理由:** 人类可读、类型明确、与 Rust 生态契合。

---

## ADR-009: 两层持久化记忆

**决策:** v0.1 的 `clawx-memory` 只实现 Agent Memory 与 User Memory 两层持久化长期记忆。

**理由:** 与 PRD 对齐，避免记忆模型范围漂移。

---

## ADR-010: Working Context 属于 Runtime

**决策:** 对话上下文窗口（Working Memory）、压缩和 Prompt 组装留在 `clawx-runtime`，不计入 `clawx-memory` 持久化记忆层。Runtime 调用 clawx-memory 获取记忆召回结果，但上下文组装由 Runtime 负责。

**理由:** 减少记忆模型概念混淆；Working Memory 是瞬时上下文管理，不属于持久化记忆范畴。

---

## ADR-011: v0.1 记忆检索用 SQLite + FTS5

**决策:** v0.1 的记忆检索采用 SQLite + FTS5，不为记忆再建一套 Qdrant + Tantivy。v0.2 可根据检索效果评估是否升级为 Qdrant 向量检索。

**理由:** 避免与知识库重复建设索引基础设施；v0.1 记忆条目数量有限（预计 < 1K），FTS5 足以满足需求。

---

## ADR-012: 文档知识库使用混合检索

**决策:** 文档知识库采用 Qdrant + Tantivy + RRF。

**理由:** 文档检索规模和复杂度比记忆更需要混合检索。

---

## ADR-013: 本地 Embedding

**决策:** 文档 Embedding 优先本地模型。

**理由:** 符合本地优先与数据主权原则。

---

## ADR-014: 三级安全执行模型

**决策:** T1 (WASM) → T2 (受限子进程) → T3 (原生宿主)。注意：T1/T2/T3 指执行级别，不同于安全架构的 L1-L12 防御层。

**理由:** 在能力和安全之间做分级平衡。

---

## ADR-015: Security 为最终边界

**决策:** `clawx-security` 拥有最终裁决权；Trust 若引入，最早在 v0.2 启用。

**理由:** Trust / Autonomy 不能越过安全上限。

---

## ADR-016: Keychain + 宿主边界注入

**决策:** 密钥存 macOS Keychain，运行时在宿主 HTTP 边界注入。

**理由:** 密钥不进入沙箱，不落盘到配置和日志。

---

## ADR-017: 哈希链审计日志

**决策:** 审计日志使用 JSONL 追加写入 + SHA-256 哈希链。

**理由:** 易写入、易校验、适合本地单机场景。

---

## ADR-018: 工作区边界内的 Vault

**决策:** 版本化与回滚只承诺 `workspace` 边界内文件。

**理由:** 与 PRD 的工作区边界一致，可审计且可恢复。

---

## ADR-019: HAL 封装 macOS 能力

**决策:** `clawx-hal` 统一封装 FSEvents、Keychain、Notification 等宿主能力。

**理由:** 便于测试与替换，不让业务层直接依赖平台 API。

---

## ADR-020: 自主性分阶段引入

**决策:** v0.1 只有 Runtime 基础护栏（最大迭代次数、Token 预算限制）；v0.2 再引入 ReAct 循环、自我反思、信任渐进等受控自主能力。

**理由:** 先保证闭环和安全，再增加自治复杂度。

---

## ADR-021: 渠道与 Skills 属于扩展执行层

**决策:** `clawx-skills`、`clawx-channel`、`clawx-scheduler` 都属于 v0.2 扩展执行层；`clawx-channel` 初期不再额外拆 `gateway` crate，IM 消息路由作为 clawx-channel 内部功能实现。

**理由:** 避免在首版预埋无收益的独立模块。

---

## ADR-022: 平台能力后置

**决策:** Artifact、Cloud Relay、账号同步、OTA 均后置到 v0.3+ / v0.4+。

**理由:** 平台能力不能反向侵入本地闭环的默认工作方式。

---

## ADR-023: 三层记忆概念模型

**决策:** 记忆系统采用三层概念模型（Working + Short-Term + Long-Term），其中 Long-Term 按作用域分为 Agent Memory 和 User Memory。Working Memory 由 Runtime 实现（见 ADR-010），Short-Term Memory 和 Long-Term Memory 由 clawx-memory 实现。

**理由:** 三层概念架构与人类记忆工作方式类比，提取和晋升机制更自然。Working Memory 实现归 Runtime 可减少概念混淆。

**详见:** [memory-architecture.md](./memory-architecture.md)

---

## ADR-024: 记忆提取采用 LLM 辅助

**决策:** 以 LLM 辅助提取为主，信号词规则检测为辅助触发器。每 3 轮对话触发一次隐式提取。

**替代方案:**
- 纯规则引擎：无法理解语义，漏提严重
- 每轮都调 LLM：Token 开销过大

**理由:** LLM 能理解对话语义，提取质量显著高于规则；通过频率控制平衡开销。

---

## ADR-025: SQLite 为记忆 Source of Truth

**决策:** SQLite 为记忆的 Source of Truth。v0.1 仅用 SQLite + FTS5（见 ADR-011）；v0.2 引入 Qdrant 向量检索后，Qdrant 为可重建的检索索引，写入时双写，Qdrant 数据丢失时可从 SQLite 重建。

**理由:** SQLite 提供事务保证和结构化查询；Qdrant 嵌入式模式下数据可能因异常损坏。

---

## ADR-026: 移动端 Cloud Relay 方案

**决策:** 移动端随行采用 Cloud Relay 方案（WSS + E2E X25519 加密），Relay 不可解密消息内容。PRD 中提及的 Tailscale/WireGuard 作为替代方案保留评估，不作为默认实现。

**理由:** Cloud Relay 零配置体验更好；E2E 加密保证数据主权。该能力依赖 v0.3+ 账号体系。

---

## ADR-027: 自主性能力集成在 clawx-runtime 而非独立 Crate

**决策:** 自主性引擎 (ReAct, Planner, Reflection 等) 作为 `clawx-runtime` 的子模块实现，而非独立 Crate。

**理由:** 自主性引擎是运行时编排的核心部分，与 Runtime 紧密耦合；独立 Crate 会引入大量跨模块调用开销；通过子模块保持代码组织清晰度。

**详见:** [autonomy-architecture.md](./autonomy-architecture.md)

---

## ADR-028: ReAct 为基础推理模式

**决策:** ReAct 为基础推理循环，Planner 作为 ReAct 内部的可选增强。复杂任务先 Plan 再在每步中 ReAct，简单任务直接 ReAct。

**理由:** ReAct 更灵活，可根据中间结果动态调整；Plan-and-Execute 在信息不完整时容易产生错误计划。

---

## ADR-029: Computer Use 优先使用 Accessibility API

**决策:** 三层操作策略：API/Script → Accessibility API → Vision (截屏+LLM)，优先使用高效精确的方法。

**理由:** 纯视觉方案 Token 消耗巨大 (每次截屏约 1-2K tokens)，且易误操作；Accessibility API 是 macOS 原生能力，精确且低开销；Vision 作为通用兜底。

---

## ADR-030: 信任等级按 Agent 独立计算

**决策:** Per-Agent 独立信任等级，而非全局共享。

**理由:** 不同 Agent 能力范围不同；用户可能信任"写作助手"的自主操作但不信任"开发者"的 Shell 执行；Per-Agent 更符合专业化分工理念。

---

## ADR-031: 预测性主动基于记忆系统

**决策:** 行为模式作为特殊类型的 Agent Memory 存储在现有记忆系统中，而非独立行为分析库。

**理由:** 复用三层记忆架构的衰减、召回、审计能力；避免引入新的存储引擎；行为模式本质上就是 Agent 对用户的"学习记忆"。
