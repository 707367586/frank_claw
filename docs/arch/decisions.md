# ClawX 架构决策记录 (ADR)

**日期:** 2026-03-18 | **对应架构:** v4.2

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

**决策:** `clawx-desktop`（Tauri app）与 `clawx-cli` 共用 `clawx-controlplane-client`，不直接依赖 runtime。

**理由:** 把"单入口"做成硬边界，而不是约定。

**历史:** 原为 `clawx-ffi`（SwiftUI UniFFI 桥接），ADR-035 决策后替换为 `clawx-desktop`（Tauri Commands）。

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

**理由:** v0.1 记忆条目数量有限（预计 < 1K），FTS5 + importance/freshness 加权排序足以满足召回需求，向量语义检索的额外收益不大；v0.2 记忆规模增长后再评估是否升级为 Qdrant 向量检索，届时可复用知识库已有的 Qdrant 实例。

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

**决策:** 平台能力按依赖链分阶段交付：Artifact + 账号体系 (v0.3) → OTA (v0.4) → Cloud Relay + 云端备份 (v0.5) → 移动端随行 + 多Agent协作 + Computer Use (v1.0+)。

**理由:** 平台能力不能反向侵入本地闭环的默认工作方式。阶段划分遵循 PRD v2.0 §4.1 路线图。

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

**理由:** Cloud Relay 零配置体验更好；E2E 加密保证数据主权。该能力依赖 v0.3 账号体系，计划在 v0.5 交付。

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

## ADR-030: 信任档案按 Agent 独立计算，并按能力向量细分

**决策:** 权限信任以 Agent 为边界独立维护，但不再只用一个单一全局等级，而是按 `knowledge_read / workspace_write / external_send / memory_write / shell_exec` 等能力维度维护信任向量。

**理由:** 不同 Agent 的可信范围不同；同一个 Agent 在“知识检索”上可能已足够可信，但在“共享记忆修改”或“Shell 执行”上仍必须严格确认。按能力拆分更符合最小权限原则，也更利于产品体验渐进开放。

---

## ADR-031: 预测性主动基于记忆系统

**决策:** 行为模式作为特殊类型的 Agent Memory 存储在现有记忆系统中，而非独立行为分析库。

**理由:** 复用三层记忆架构的衰减、召回、审计能力；避免引入新的存储引擎；行为模式本质上就是 Agent 对用户的"学习记忆"。

---

## ADR-032: 主动任务统一采用 Task / Trigger / Run 三段模型

**决策:** 主动任务不再使用单表 `scheduled_tasks` 的轻量模型，而统一采用：

- `Task`：描述“做什么”
- `Trigger`：描述“何时做”
- `Run`：描述“一次真正发生的执行”

通知结果和权限变化作为独立记录建模。

**理由:** 单表模型无法支撑恢复、去重、重试、通知抑制和执行历史。Task / Trigger / Run 能更清晰地覆盖 PRD 对主动任务“可暂停、可恢复、可追溯、可反馈”的要求。

---

## ADR-033: clawx-service 作为自主性的组合根

**决策:** `clawx-runtime` 的自主性模块通过 `clawx-types` 中定义的 `TaskRegistryPort`、`NotificationPort` 等 Trait 与外部交互；`clawx-service` 作为组合根负责装配 `clawx-scheduler`、`clawx-channel`、`clawx-hal` 的具体实现。

**理由:** 这样可以保持 Runtime 作为编排核心，同时避免它对 `scheduler`、`channel` 产生直接依赖，消除隐藏的双向耦合，也与现有“service-owned control plane”方向一致。

---

## ADR-034: 主动通知必须经过 Attention Policy

**决策:** 主动任务执行完成后，不默认“有结果就通知”，而必须先经过独立的 Attention Policy 判断是否立即推送、汇总后推送、仅记录历史或直接抑制。

**理由:** PRD 对主动式 Agent 的核心要求不是”更多通知”，而是”少打扰、真推进、可衡量”。Attention Policy 是把这一要求工程化的必要组件，也是把负反馈率控制在目标阈值内的关键手段。

---

## ADR-035: Tauri 替代 SwiftUI 作为桌面 GUI 方案

**决策:** 使用 Tauri v2 + React + TypeScript 替代原定的 SwiftUI + UniFFI 方案构建桌面 GUI。删除 `clawx-ffi` crate，新建 `apps/clawx-desktop`（Tauri app）。

**替代方案:**
- SwiftUI + UniFFI（原方案）：macOS 原生体验最佳，但开发门槛高、生态封闭、跨平台能力为零
- Electron：跨平台成熟，但打包 Chromium 导致体积 > 150MB，内存占用高
- Flutter：跨平台但 Rust FFI 集成不成熟，桌面端生态较弱

**理由:**
1. Tauri v2 使用系统 WKWebView，不打包浏览器引擎，最终 .app 体积约 20-40MB
2. React + TypeScript 生态成熟，UI 组件库丰富，开发效率显著高于 SwiftUI
3. Tauri Commands 是原生 Rust 函数，与现有 Workspace 天然集成，无需额外 FFI 抽象层
4. 架构核心不变：Tauri app 仍通过 `clawx-controlplane-client` 访问 `clawx-service`，符合 ADR-003/004
5. 保留跨平台潜力（Linux/Windows），但当前阶段仅面向 macOS

**权衡:**
- 内存占用略高于 SwiftUI（WKWebView 基线约 80-120MB），PRD 空闲内存预算从 300MB 调整为 400MB
- 部分 macOS 原生体验（如系统级弹窗）需通过 Tauri 插件实现，体验接近但非完全等同
- 放弃 SwiftUI 后，未来 iOS 移动端随行（v1.0+）需独立开发原生 Swift app 或采用 React Native

**废弃:** `clawx-ffi` crate 彻底删除，不再维护。

---

## ADR-037: 2026-04-20 删除 Rust 后端，将 picoclaw 源码 vendor 进本仓库作为新后端

**状态:** Accepted — SUPERSEDES：ADR-001 / 002 / 003 / 004 / 005 / 008 / 009 / 010 / 011 / 014 / 015 / 016 / 017 / 018 / 019 / 020 / 023 / 024 / 025 / 027 / 028 / 029 / 030 / 031 / 032 / 033 / 034 / 035 / 036。仅 ADR-006 / 007 / 012 / 013 / 021 / 022 / 026 作为历史记录保留。

**修订历史:**

- 2026-04-20 v1：决定"以官方二进制 / docker-compose 形式消费 picoclaw，不 fork、不维护补丁"。
- 2026-04-20 **v2（当前）**：实证发现已发布的 `:v0.2.6` 镜像缺失计划所依赖的 `/api/*` REST 端点和 launcher 子命令；`web/frontend` + 完整 `/api/*` 仅在主分支源码中存在。结论：**只能 vendor 源码**。同时用户明确允许"修改 picoclaw 代码"，故放弃"不 fork"约束。

**背景:**

ClawX 自研 Rust 后端在 v0.3 完成了"agentic tool-use loop Phase 1"（ADR-036）。同期上游 [sipeed/picoclaw](https://github.com/sipeed/picoclaw) 已实现：单文件 Go 二进制、内置 30+ LLM provider、原生 MCP / Skills / Hooks / SubTurn / Channels 抽象、MIT 协议。继续维护 17 个 Rust crate 的边际成本远高于复用 picoclaw 的源码。

**决策（v2）:**

1. **删除整个 Rust workspace**：所有 `crates/*`、`apps/clawx-service`、`apps/clawx-cli`、`Cargo.{toml,lock}`、`rust-toolchain.toml`、`clippy.toml`、`rustfmt.toml`、`target/` 全部清除。
2. **picoclaw 源码 vendor 进本仓库**，目录为 `backend/`：
   - 来源：`https://github.com/sipeed/picoclaw` 在 commit SHA `8461c996e5ad2f20801622a8eeec931f8966a066`（2026-04-20 main HEAD）。
   - 方式：直接平铺拷贝（不用 git subtree / submodule），`backend/UPSTREAM.md` 记录 SHA、原始 license（MIT）、手动同步上游变更的方法。
   - **允许自由修改**：本仓库即为该 picoclaw 副本的事实 fork。当上游协议或行为不满足前端需求时，直接在 `backend/` 内改 Go 代码 + 写 Go 测试，不再上推上游。
3. **不使用 docker**：本地开发以 `go run ./backend/cmd/picoclaw-launcher` 启动后端，前端 `pnpm dev` 单独跑；通过 Vite proxy 把 `/api/*` 与 `/pico/ws` 转发到后端。Production 单二进制：`go build -o build/picoclaw-launcher ./backend/cmd/picoclaw-launcher` + `pnpm build` 输出的静态资产由 launcher 自己嵌入或 nginx 托管。
4. **前端从 Tauri 桌面 app 退化为纯 Web 前端**：保留 `apps/clawx-gui/src/` 的对话相关 React 组件；删除 `src-tauri/`、`@tauri-apps/cli`、`tauri` 脚本。
5. **协议层换血**：弃用自研的 40+ REST 端点 + SSE。前端通过 picoclaw launcher 暴露的：
   - REST：`/api/pico/token`、`/api/sessions`、`/api/sessions/:id`、`/api/skills`、`/api/tools`
   - WebSocket：`/pico/ws?session_id=…`，鉴权用 `Sec-WebSocket-Protocol: token.<…>` 子协议
6. **删除以下产品能力**（picoclaw launcher 不暴露给客户端，或与新协议不兼容）：
   - Agent 管理（picoclaw 把"角色 / 人格"当作 Skill / 配置）
   - 任务调度（picoclaw cron 不向客户端暴露管理面）
   - Vault 快照 / 工作区版本化
   - 知识库（KB / Qdrant / Tantivy）
   - 持久化记忆（Long-Term / Short-Term / Working）
   - Tool 审批 UI（Pico WS 协议无 `tool_use` / `approval` 消息类型）
   - 模型路由 / Provider 管理 UI（改由 `~/.picoclaw/config.json` + `.security.yml` 配置）
   - macOS sandbox-exec / Keychain / FSEvents（picoclaw 用 Go 实现自己的安全模型）

**关键权衡（v2）:**

- **fork 而非外部依赖**：好处是协议、行为、错误格式都由我们控制；前端有任何不满意可以直接改 Go。代价是手动同步上游需要 cherry-pick / 三方合并。在 `backend/UPSTREAM.md` 写明同步流程，并约定 `backend/PATCHES.md` 记录每个本地改动的 rationale，避免日后失忆。
- **协议失配**：picoclaw 不流式返回 token 增量，只在每条消息完成时下发一条 `message.create`；中间过程通过 `payload.thought:true` 表达。前端从 "delta accumulator" 重写为 "message-id merge store"。如未来需要细粒度流式，可在 `backend/pkg/channels/pico/protocol.go` 自行扩展。
- **审批 UI**：Pico WS 协议未定义 tool-use / approval 消息类型，因此首版不实现审批 UI。**未来若要恢复**，可以在 `backend/` 内扩 Go 协议 + 在前端 `chat-store` 中加分支处理；本地修改属于已被允许的范围。
- **不再"跟随上游版本号"**：上游 picoclaw 的版本号失去意义，本仓库的版本号 = ClawX Web 自身的版本号。`backend/UPSTREAM.md` 单独记录上游基线 SHA 与下次同步建议。
- **跨平台得失**：失去 macOS Tauri 原生壳与 Keychain，获得跨平台浏览器访问能力。
- **数据迁移**：本地 SQLite 对话 / 记忆**不迁移**（v0.x 早期阶段，无真实用户数据）。

**影响面:**

- `docs/arch/architecture.md` v5.0（picoclaw vendored 形态）；
- `docs/arch/api-design.md` v5.0（我们拥有的契约，不是消费上游）；
- `docs/arch/{autonomy,memory,security,data-model,crate-dependency-graph}.md` 标记为 **DEPRECATED**；
- `docs/prd/*` 与新形态不一致，本 ADR 不处理 PRD；
- `apps/clawx-gui` 内 `pages/{Agents,Tasks,Knowledge,Contacts}.tsx`、`components/{Agent*,Task*,Knowledge*,Source*,Artifacts*,AddProvider*,ModelProvider*}.tsx`、`lib/agent-conv-memory.ts` 全部删除；
- `lib/api.ts`、`lib/types.ts`、`lib/chat-stream-store.ts`、`lib/store.tsx` 全部重写。

**版本号:** 完成迁移后版本跳到 **v0.4**；`v0.3` tag 已存档为 Rust 时代的最后稳定版（同时存在 `release_v0.3` 备份分支）。

---

## ADR-036: 2026-04-19 Agentic tool-use loop (Phase 1)

> **状态:** SUPERSEDED by ADR-037。Rust 实现已删除，tool-use 由 picoclaw 内部承担，前端不再可见。本 ADR 作为历史记录保留。



**背景:** `agent_loop::run_turn` 以前从不向 LLM 传递 `tools`，Agent 只能输出文本，无法让它在本地创建文件夹、列目录、执行命令。目标是对标 picoclaw：把 tool loop、内置工具、审批门、macOS 沙箱组合起来落地。

**决策:**

- 新增 `clawx-tools` crate，定义 `Tool` trait + `ToolRegistry` + `ToolExecCtx`。
- 扩展 `clawx-types::llm::Message`，新增结构化 `blocks: Vec<ContentBlock>`（`Text` / `ToolUse` / `ToolResult`），与旧的 `content: String` 并存，保证历史代码可编译。
- 三家 LLM provider 统一补齐 tool wire：Anthropic 的 `tool_use` / `tool_result` content block、OpenAI-compat 的 `tool_calls` 数组（ZhipuAI 委托 OpenAI 模块实现）。`is_error: false` 强制不上线（`skip_serializing_if`）；`finish_reason = "sensitive"` 映射为 `StopSequence` 保留 GLM 语义。
- `runtime::tool_loop::run_with_tools` 作为循环驱动：注入 `registry.definitions()`、重写 assistant content blocks、把每次工具结果作为 `role: Tool` 追回消息列表、在 `StopReason != ToolUse` 时返回。当 `max_iterations` 触顶时返回 `ClawxError::Tool`。
- `agent_loop::run_turn` 根据 `(runtime.tools, runtime.approval, runtime.workspace)` 是否齐备二选一：齐备走 tool loop，缺任何一个则保持原 single-call 行为（向后兼容）。
- 内置工具：`fs_read` / `fs_write` / `fs_mkdir` / `fs_list` / `shell_exec`。所有路径经 `resolve_in_workspace` 双重把关——先词法拒绝 `..` 与绝对路径越界，再 canonicalize 最深存在祖先，拦截"workspace 内预存的指向外部的 symlink"。
- `shell_exec` 仅 macOS 实现：包一层 `/usr/bin/sandbox-exec -p <profile>`，profile 默认拒网络、仅允许在 workspace / TMPDIR 下写。非 macOS 返回 `unsupported`。
- `RuleApprovalGate` 三档：`auto` / `prompt` / `deny`，按 `tool × path_glob` 规则链表匹配，用户规则前插优先。`default_claw_code_style()` 基线：读类 auto，写+shell prompt。
- `ChannelPromptGate` 把 `prompt` 档挂到 `POST /tools/approval/:id`，GUI 决策写回 oneshot。

**关键权衡:**

- 不引入 `globset`：`glob_match` 手写 `*` / `?` 递归匹配器，规则数据面由受信 config 提供，LLM 无法投毒路径 glob，O(2^n) 最坏复杂度可接受。
- 不做 streaming + tool_use：三家 provider 的 `stream()` 在 `request.tools.is_some()` 时 early-return 错误。Phase 2 的 SSE 解析补齐后再处理。
- 沙箱 profile 里放宽 `subpath "/private/var/folders"` 是为 TMPDIR，`tempfile::tempdir()` canonicalize 后正好落在这条。
- GUI 审批端点当前挂在 auth middleware 之外（loopback UDS/TCP 触达），遵循现有 API 的安全模型。

**影响面:**

- `Message` 新字段 `blocks` 通过 `#[serde(default, skip_serializing_if = "Vec::is_empty")]` 兼容旧 JSON；所有 `Message { ... }` 构造点加 `blocks: vec![]` 即可。
- Phase 2 计划（hook / SubTurn / steering / MCP 客户端 / markdown skills / GUI 审批对话框 / streaming + tool_use）全部可以构建在这套 surface 之上，不需要回头动 provider 或 content block。

**验证:** 整套 `cargo test --workspace` 绿；新增 E2E 测试 `agent_creates_folder_via_tool_use` 用 scripted LLM 在临时 workspace 里实际创建了目录；`shell_blocks_write_outside_workspace` 用真 `sandbox-exec` 拦住了 `$HOME` 写入。
