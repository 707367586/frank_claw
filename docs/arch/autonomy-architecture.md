# ClawX 自主性架构设计 (Autonomy Architecture)

> ⚠️ **DEPRECATED — 2026-04-20**
>
> 自 [ADR-037](./decisions.md#adr-037-2026-04-20-全面迁移至-picoclaw-后端删除全部-rust-代码) 起，本仓库已删除全部 Rust 后端，自主性能力由 [picoclaw](https://github.com/sipeed/picoclaw) 在服务端承担，前端不再可见也不可控。
> 本文档仅作 **历史参考**，与 v5.0 形态不一致。当前架构请见 [architecture.md](./architecture.md)。

---

**版本:** 3.1（已废弃）
**日期:** 2026年3月18日
**对应 PRD:** v2.0 §2.8 主动式 Agent
**对应架构:** v4.2 clawx-runtime
**关联 ADR:** ADR-015, ADR-020, ADR-027, ADR-028, ADR-030, ADR-032, ADR-033, ADR-034

---

## 1. 设计目标与边界

### 1.1 问题定义

当前 ClawX 的 Agent 已能完成单轮对话，但距离 PRD 中“少打扰、真推进、可衡量地帮用户完成更多事”的主动式 Agent 仍有明显差距。核心缺口有三类：

1. **多步执行能力不足**：复杂目标仍容易退化为“一问一答”，无法稳定完成搜索、分析、生成、写入等连续步骤。
2. **后台任务不够产品化**：只有“定时触发”还不够，任务还必须可暂停、可恢复、可抑制、可追溯。
3. **自主性与安全/体验耦合不清**：如果没有权限渐进、通知抑制和可靠恢复，自主性会变成打扰源和风险源。

自主性架构的目标是：**让 Agent 在安全围栏内完成多步任务，并通过可恢复、可解释、可抑制的任务系统执行主动触达。**

### 1.2 设计原则

| 原则 | 说明 |
|------|------|
| 实用优先 | 先把用户最常用的后台执行和多步任务做稳定，不追求学术完备性 |
| 受控执行 | 所有自主行为都在 `clawx-security` 安全边界内，且可暂停、可取消、可追溯 |
| 可靠优先 | 主动任务必须有状态机、重试、恢复和去重，而不只是“定时调一次” |
| 少打扰 | 主动通知必须经过价值判断、冷却和抑制策略，不能把用户当消息队列 |
| 渐进开放 | 权限不是全开或全关，而是按 Agent、按能力范围逐步放宽 |
| 最小增量 | 自主性仍然是 `clawx-runtime` 的子模块；跨模块交互通过 Trait Port 和 EventBus 完成 |

### 1.3 非目标

- 不追求完全自主。Agent 不自行设定高层目标，始终服务于用户明确意图或已批准的任务。
- 不在 v0.2 就交付预测式“全自动生活管家”。`上下文感知` 和 `策略触达` 在 v0.2 只保留架构挂点，不作为默认开放能力。
- 不把 raw chain-of-thought 变成产品对象。系统只展示结构化步骤说明，不暴露原始推理文本。

### 1.4 能力与阶段对齐

| 能力 | 阶段 | 说明 |
|------|------|------|
| 基础护栏 | **v0.1** | 最大迭代次数、Token 预算限制、基础循环检测 |
| 多步工具调用 | **v0.2** | Executor 多步执行 + 安全围栏 + 中断/确认 |
| 主动任务系统 | **v0.2** | `time/event` 触发的后台任务、执行历史、恢复与通知 |
| 权限渐进 | **v0.2** | 按 Agent 独立维护，按能力向量细分 |
| Attention Policy | **v0.2** | 反馈抑制、冷却、静默时段、去重与汇总 |
| `context/policy` 触发 | **v0.3+** | 先 Shadow Mode 评估，再决定是否对用户开放 |

> **与 PRD 的对齐说明：**
> - PRD §2.8 给出了四类触发机制，但路线图 §4.1 在 v0.2 明确只要求“定时任务 + 事件驱动”。
> - 因此本架构将 `time/event` 作为 v0.2 交付范围；`context/policy` 作为预留触发类型和后续演进方向，不反向阻塞 v0.2。

---

## 2. 架构总览

### 2.1 核心概念

自主性架构由 6 个核心概念组成：

| 概念 | 作用 |
|------|------|
| **Execution** | 一次具体执行，从目标出发，通过多步工具调用完成任务 |
| **Task** | 用户可管理的长期任务定义，描述“做什么” |
| **Trigger** | 任务触发器，描述“何时做” |
| **Run** | Task 的一次实际执行实例，拥有独立状态机和恢复信息 |
| **Attention Policy** | 在通知前判断“是否值得打扰用户” |
| **Permission Profile** | 按 Agent、按能力维度维护的信任档案 |

两层关系如下：

- **Task** 是稳定定义，包含目标、通知策略、默认执行配置。
- **Trigger** 是 Task 的触发入口，可有多个。
- **Run** 是一次执行实例，用于承载状态、checkpoint、重试、反馈和通知结果。

### 2.2 在 Runtime 中的位置

```
┌────────────────────────────────────────────────────────────────────┐
│                        clawx-runtime                              │
│                                                                    │
│  ┌──────────────────────────────────────────────────────────────┐  │
│  │                   Autonomy Module (v0.2)                     │  │
│  │                                                              │  │
│  │  ┌──────────────┐   ┌──────────────┐   ┌──────────────────┐  │  │
│  │  │  Executor    │   │ Task Manager │   │ Attention Policy │  │  │
│  │  │ 多步执行循环 │   │ 任务/Run状态 │   │ 通知价值判断      │  │  │
│  │  └──────┬───────┘   └──────┬───────┘   └────────┬─────────┘  │  │
│  │         │                  │                    │            │  │
│  │  ┌──────┴──────────────────┴────────────────────┴─────────┐  │  │
│  │  │                   Permission Gate                      │  │  │
│  │  │   能力向量 │ 风险分级 │ 自动放行/确认/拒绝决策         │  │  │
│  │  └────────────────────────────────────────────────────────┘  │  │
│  └──────────────────────────────────────────────────────────────┘  │
│                                                                    │
│  Agent Lifecycle │ Conversation │ Tool Dispatch │ Working Memory   │
└────────────────────────────────────────────────────────────────────┘
            │                     │                        │
            ▼                     ▼                        ▼
    clawx-security         TaskRegistryPort        NotificationPort
    (执行边界/DLP)         (service 注入)           (service 注入)
            │                     │                        │
            ▼                     ▼                        ▼
      clawx-scheduler       SQLite / EventBus      clawx-channel / HAL
      (time/event)                                 (IM/桌面/文件)
```

### 2.3 组合边界

为避免 `runtime` 与 `scheduler/channel` 出现隐藏双向耦合，采用如下边界：

- `clawx-runtime` 只依赖 `clawx-types` 中定义的 Port Trait。
- `clawx-service` 是**组合根**，负责将 `TaskRegistryPort`、`NotificationPort` 的具体实现注入给 Runtime。
- `clawx-scheduler` 负责 `time/event` 触发源，并通过 EventBus 把触发事件投递给 Runtime。
- `clawx-channel` 和桌面通知适配器只负责递送，不参与任务决策。

### 2.4 三条执行路径

**路径 A：交互式多步执行**

```
User Input
  → Runtime 意图评估
  → 简单问答: 线性对话链路
  → 复杂任务: Executor
  → Step Summary 流式展示
  → 最终结果 / 待确认操作
```

**路径 B：后台主动任务（v0.2）**

```
Scheduler(time/event) → EventBus → Task Manager
  → 创建 Run
  → Executor
  → Attention Policy
  → NotificationPort
```

**路径 C：上下文/策略信号（v0.3+）**

```
Memory / Runtime Signal → 候选触发
  → Shadow Mode 评估采纳率与负反馈率
  → 达标后再开放给用户
```

---

## 3. Executor：多步工具调用循环

### 3.1 核心思路

Executor 将执行从“单次调用”扩展为**状态化多步循环**：

1. 读取当前 Goal、已完成步骤、可用工具和预算。
2. 生成下一步行动计划。
3. 经 `Permission Gate` 判定后执行工具。
4. 记录结构化步骤摘要与结果。
5. 判断完成、继续、等待确认或失败。

### 3.2 Execution 状态机

| 状态 | 说明 |
|------|------|
| `queued` | 已创建，等待调度执行 |
| `planning` | 计算下一步行动 |
| `running` | 工具执行或模型处理中 |
| `waiting_confirmation` | 遇到需确认操作，等待用户决定 |
| `completed` | 正常完成 |
| `failed` | 明确失败，带失败原因 |
| `interrupted` | 被用户中断、超时或系统恢复流程中断 |

后台任务的 Run 必须把状态持久化，不能只存在内存里。

### 3.3 安全围栏

| 约束 | 默认规则 | 触发行为 |
|------|---------|---------|
| 步数上限 | 默认 10 步，可配置 1-25 | 超限后终止并返回已完成结果 |
| Token 预算 | 80% 预警，100% 强制终止 | 记录消耗并结束当前 Run |
| 循环检测 | 连续 3 次相同工具调用或等价结果 | 暂停或请求用户指导 |
| 超时 | 前台默认 5 分钟，后台默认 30 分钟 | 标记 `interrupted` 并保存 checkpoint |
| checkpoint | 每步提交后持久化一次 | 用于重启恢复和重试 |
| 去重键 | 每个 Run 拥有 `idempotency_key` | 防止定时误触发导致重复执行 |

### 3.4 解释性输出

GUI 和审计系统只记录**结构化步骤摘要**，不记录 raw thought。

每步输出结构如下：

| 字段 | 说明 |
|------|------|
| `step_no` | 步骤编号 |
| `action` | 执行动作，如“搜索论文”“写入报告” |
| `tool` | 使用的工具或能力 |
| `evidence` | 输入来源或命中的材料摘要 |
| `risk_reason` | 需要确认时的风险说明 |
| `result_summary` | 本步结果摘要 |

### 3.5 意图评估

进入 Executor 前，Runtime 对用户请求做一次轻量判定：

- `simple`: 单次回答即可完成
- `assisted`: 需要 1 次工具调用
- `multi_step`: 需要 2 步及以上，进入 Executor

v0.2 允许使用 LLM 进行判定，但判定结果必须转成结构化标签，不能只依赖 Prompt 中的隐式约定。

---

## 4. Task Manager：主动任务系统

### 4.1 核心职责

Task Manager 是任务系统的控制面，不负责底层调度源。它的职责是：

1. 管理 Task 生命周期
2. 管理 Trigger 配置
3. 为每次触发创建 Run
4. 维护 Run 状态机、重试和恢复
5. 记录结果、反馈和抑制状态
6. 调用 Attention Policy 决定是否通知

### 4.2 Trigger 模型

| 触发类型 | 阶段 | 来源 | 说明 |
|---------|------|------|------|
| `time` | v0.2 | Cron / 一次性时间 | PRD 要求的定时任务 |
| `event` | v0.2 | FSEvents / 磁盘告警 / 网络变化 | PRD 要求的事件驱动 |
| `context` | v0.3+ | 记忆、截止日期、上下文状态 | 先 Shadow Mode |
| `policy` | v0.3+ | 规则匹配、关键词命中 | 先 Shadow Mode |

> v0.2 的 GUI 和 API 只开放 `time/event`。`context/policy` 只保留数据模型与调度接口，不默认开放给用户。

### 4.3 Task 定义

每个主动任务包含以下要素：

| 字段 | 说明 |
|------|------|
| 名称 | 用户可见名称 |
| 关联 Agent | 执行该任务的 Agent |
| Goal | 传给 Executor 的标准化目标文本 |
| Trigger 集 | 一个任务可有多个触发器 |
| 默认执行配置 | `max_steps`、`timeout_secs`、预算上限等 |
| 通知策略 | 渠道、静默时段、汇总策略、冷却时间 |
| 创建来源 | `conversation/manual/suggestion/imported` |
| 生命周期 | `active/paused/archived` |

### 4.4 Run 状态与可靠性

每次触发都生成一个 `Run`，用于承载“真正发生的一次执行”。

| 能力 | 作用 |
|------|------|
| `attempt` | 标识第几次执行尝试 |
| `lease_expires_at` | 防止 service 重启后同一 Run 被重复消费 |
| `checkpoint` | 保存已完成步骤和中间产物指针 |
| `retry_policy` | 退避、最大重试次数 |
| `notification_status` | 跟踪通知是否已递送、失败或被抑制 |
| `feedback` | 记录采纳、忽略、拒绝、不再提醒、降低频率 |

这使 Task Manager 从“调一下就结束”的轻量任务系统，升级为**可恢复、可回溯、可运营**的后台执行系统。

### 4.5 对话创建任务

用户在对话中通过自然语言创建任务时，流程如下：

```
用户: "帮我每天早上 8 点检查 arXiv 上 AI Safety 的新论文，整理摘要发飞书"

  → Runtime 解析结构化任务:
    trigger = time("0 8 * * *")
    goal = "检查 arXiv 上 AI Safety 新论文，整理摘要"
    notifications = ["lark"]

  → 系统展示确认卡片:
    名称 / 触发时间 / 通知方式 / 首次执行权限范围

  → 用户确认后:
    创建 Task + Trigger
    通过 TaskRegistryPort 注册到 Scheduler
```

### 4.6 反馈与抑制策略

用户可对每次主动触达给出反馈：

| 反馈类型 | 效果 |
|---------|------|
| `accepted` | 正常记录，增加该类通知的可信度 |
| `ignored` | 进入冷却统计；连续 3 次忽略可自动暂停 |
| `rejected` | 计入负反馈率，并下调相关能力分数 |
| `mute_forever` | 任务归档，不再触发 |
| `reduce_frequency` | 调整 Trigger 频率或冷却窗口 |

`负反馈率 = (rejected + mute_forever) / 总主动触达次数`

PRD 要求该指标持续低于 `15%`。因此 Task Manager 必须把反馈直接接入后续调度和通知决策，而不是只做展示。

---

## 5. Attention Policy：少打扰机制

### 5.1 为什么需要独立一层

如果任务系统只负责“执行成功就通知”，主动式 Agent 会迅速演化成噪音系统。Attention Policy 专门负责回答一个问题：

**这次结果值得现在打扰用户吗？**

### 5.2 决策输入

Attention Policy 在通知前综合以下信号：

| 信号 | 说明 |
|------|------|
| 触发类型 | `time/event/context/policy` |
| 结果等级 | 失败、异常、普通信息、强相关发现 |
| 历史反馈 | 最近的采纳率、忽略率、负反馈率 |
| 冷却窗口 | 同类通知是否刚发过 |
| 静默时段 | 用户是否处于 quiet hours |
| 汇总策略 | 是否应延迟到 digest 中统一发送 |

### 5.3 输出决策

| 决策 | 行为 |
|------|------|
| `send_now` | 立即通过选定渠道发送 |
| `send_digest` | 进入稍后汇总通知 |
| `store_only` | 仅记录到任务历史，不推送 |
| `suppress` | 本次直接抑制，并记录原因 |

### 5.4 v0.2 最小实现

v0.2 至少实现以下规则：

- 静默时段内普通结果默认不立即推送
- 相同任务在冷却窗口内不重复推送
- 连续 3 次 `ignored` 自动暂停并提示用户
- `mute_forever` 直接归档
- `reduce_frequency` 自动调整 Trigger 或冷却参数

---

## 6. Permission Gate：权限渐进系统

### 6.1 核心思路

权限渐进不再只依赖“做成了多少任务”，而是维护**按 Agent 独立、按能力维度拆分**的信任档案。

一个 Agent 可能在“知识检索”上高度可信，但在“修改共享记忆”或“执行 Shell”上仍然必须严格确认。

### 6.2 能力向量

| 能力维度 | 典型动作 |
|---------|---------|
| `knowledge_read` | 知识库检索、网页搜索、文件读取 |
| `workspace_write` | 工作区内文件写入、创建目录、更新产物 |
| `external_send` | 发送 IM、调用外部 API、主动通知外发 |
| `memory_write` | 修改 Agent Memory / User Memory |
| `shell_exec` | Shell、脚本执行、系统级操作 |

### 6.3 等级定义

每个能力维度都维护 `L0-L3` 四级：

| 等级 | 说明 |
|------|------|
| `L0 Restricted` | 默认全部需确认 |
| `L1 Read Trusted` | 只对低风险读取类自动放行 |
| `L2 Workspace Trusted` | 可自动执行工作区内低中风险写入 |
| `L3 Channel Trusted` | 可自动进行部分外发类动作，但仍受安全上限限制 |

### 6.4 风险映射

| 风险 | 典型操作 | 自动放行条件 |
|------|---------|-------------|
| `read` | 文件读取、检索、搜索 | `knowledge_read >= L1` |
| `write` | 工作区写入、生成产物 | `workspace_write >= L2` |
| `send` | IM 推送、外部 API | `external_send >= L3` |
| `memory_low` | 低重要性 Agent Memory 写入 | `memory_write >= L2` |
| `memory_high` | User Memory / 高重要性共享记忆修改 | 永远需确认 |
| `danger` | Shell、删除文件、系统配置 | 永远需确认 |

### 6.5 升降级规则

升级和降级都按能力维度单独计算。输入信号包括：

- 该能力范围内的成功执行数
- 用户确认通过率
- 被拒绝率与主动负反馈率
- 安全事件记录（DLP、权限越界、失败重试过多）

关键约束：

- 新建 Agent 默认为所有能力 `L0`
- **L0 Agent 可以创建任务，但必须经过显式确认**
- `danger` 和 `memory_high` 在任何等级下都需要确认
- 安全事件触发时，只降级相关能力维度，不必整体清零

---

## 7. 与安全架构的集成

### 7.1 安全边界

自主执行受 `clawx-security` 纵深防御约束：

| 安全层 | 与自主性的交互 |
|--------|-------------|
| L2 分级执行 | Executor 中的工具调用仍然走 T1/T2/T3 |
| L4 权限能力模型 | Permission Gate 只能在 L4 允许的能力范围内继续细分 |
| L5 DLP | 每步工具返回、外发通知、云端 LLM 出站都要扫描 |
| L7 路径隔离 | 工作区边界仍是文件写入上限 |
| L9 循环守卫 | 扩展为支持 Run 级步数、超时、去重与恢复 |
| L12 审计日志 | 任务触发、Run 状态变化、权限变更、通知抑制全部记录 |

### 7.2 高风险记忆改写

`User Memory` 和高重要性共享记忆修改必须视为高风险动作：

- 进入 `memory_high` 风险级别
- 弹出原生确认
- 记录审计
- 写入 `memory_audit_log`

这样可与 [memory-architecture.md](./memory-architecture.md) 的共享记忆审计规则保持一致。

### 7.3 审计日志扩展

| 事件类型 | 记录内容 |
|---------|---------|
| `task_created` | 任务来源、初始 Trigger、初始通知策略 |
| `task_triggered` | 任务 ID、触发器 ID、触发类型、触发时间 |
| `run_state_changed` | Run ID、旧状态、新状态、原因 |
| `execution_step` | 结构化步骤摘要，不含 raw thought |
| `confirmation_required` | 待确认操作、风险等级、影响范围 |
| `confirmation_result` | 用户批准/拒绝 |
| `notification_decision` | send/digest/store/suppress 及原因 |
| `task_feedback` | Run ID、反馈类型 |
| `permission_change` | Agent、能力维度、旧/新等级、触发原因 |

---

## 8. 数据存储

自主性相关数据存储在 SQLite 主数据库中，权威模型如下：

| 表 | 用途 | 关键字段 |
|----|------|---------|
| `tasks` | 任务定义 | agent_id, name, goal, lifecycle_status, source_kind |
| `task_triggers` | 触发器 | task_id, trigger_kind, trigger_config, status, next_fire_at |
| `task_runs` | 一次执行实例 | task_id, trigger_id, status, attempt, checkpoint, idempotency_key |
| `task_notifications` | 通知递送结果 | run_id, channel_kind, delivery_status, suppression_reason |
| `permission_profiles` | Agent 权限档案 | agent_id, capability_scores, safety_incidents |
| `permission_events` | 权限变更审计 | agent_id, capability, old_level, new_level, reason |

设计约束：

- `tasks` 负责“定义”
- `task_triggers` 负责“何时触发”
- `task_runs` 负责“真正发生的一次执行”
- `task_notifications` 负责“通知是否发出”

这样数据模型能同时支撑恢复、幂等、反馈统计和任务面板。

---

## 9. 用户体验

### 9.1 交互式多步执行

GUI 展示结构化进展，而不是 raw thought：

```
Step 1/10  搜索 arXiv 上本周 AI Safety 论文
来源: web_search
结果: 找到 12 篇候选论文

Step 2/10  筛选最近 7 天且高相关的 5 篇
来源: rerank + rule filter
结果: 已筛出 5 篇

Step 3/10  生成摘要报告
结果: 正在流式输出...
```

用户可随时中断；中断后保留已完成步骤和中间产物引用。

### 9.2 任务管理面板

任务列表至少展示：

- 名称、关联 Agent、Trigger 数量
- 上次/下次执行时间
- 生命周期状态与最近一次 Run 状态
- 采纳率、忽略率、负反馈率
- 当前抑制状态和冷却窗口

### 9.3 权限与通知可见性

Agent 详情页展示：

- 各能力维度的当前等级
- 最近一次降级原因
- 哪些动作会自动放行，哪些仍需确认
- 当前 Attention Policy 的主要规则

---

## 10. 模块变更清单

```
❶ clawx-runtime (修改)
   ├── 新增: autonomy/
   │   ├── executor.rs
   │   ├── task_manager.rs
   │   ├── attention_policy.rs
   │   ├── permission.rs
   │   └── mod.rs
   ├── 修改: dispatcher.rs
   └── 修改: lifecycle.rs

❷ clawx-types (修改)
   └── 新增: autonomy.rs
       - TaskRegistryPort
       - NotificationPort
       - PermissionProfile / Task / Run / Trigger 等共享类型

❸ clawx-security (修改)
   └── 修改: L9 循环守卫，扩展到 Run 级约束

❹ clawx-api (修改)
   └── 新增: /tasks、/task-runs、/agents/:id/permission-profile 端点

❺ 数据库迁移
   └── 新增: tasks, task_triggers, task_runs, task_notifications,
            permission_profiles, permission_events

❻ clawx-service (修改)
   └── 作为组合根装配 TaskRegistryPort 和 NotificationPort
```

依赖关系：

```
clawx-runtime/autonomy
  ├── clawx-types      (共享类型 + Port Trait)
  ├── clawx-llm        (LLM 调用)
  ├── clawx-memory     (记忆召回)
  ├── clawx-kb         (知识检索)
  ├── clawx-security   (权限检查、DLP)
  └── clawx-vault      (工作区写入前版本化)

clawx-service
  ├── clawx-runtime
  ├── clawx-scheduler  (TaskRegistryPort 实现 + EventBus 触发)
  ├── clawx-channel    (NotificationPort 一部分实现)
  └── clawx-hal        (桌面通知 / 系统信号)
```

---

## 11. 阶段交付与验收

### 11.1 v0.1 交付

| 项 | 内容 |
|----|------|
| 基础护栏 | `max_steps`、Token 预算限制、基础循环检测 |
| 验收 | 超出步数或预算时正确终止，不出现无限循环 |

### 11.2 v0.2 交付

| 项 | 内容 |
|----|------|
| Executor | 多步执行循环、步骤摘要、确认/中断 |
| Task Manager | Task/Trigger/Run 模型、恢复、反馈与统计 |
| Trigger | 对外开放 `time/event` |
| Attention Policy | 冷却、静默时段、忽略自动暂停、频率下调 |
| Permission Gate | 按能力维度的 L0-L3 渐进权限 |
| 安全集成 | Run 级审计、DLP、共享记忆高风险确认 |

### 11.3 验收标准

| 项目 | 标准 |
|------|------|
| 多步执行 | 在 10 步以内完成“搜索 + 分析 + 生成报告”类任务 |
| 可靠性 | service 重启后可恢复 `running/waiting_confirmation` 的 Run |
| 主动任务 | `time/event` 任务按时触发，并具备去重与重试 |
| 对话创建 | 自然语言正确解析为 Task + Trigger + 通知策略 |
| 反馈机制 | `mute_forever` 归档；`reduce_frequency` 调整触发器或冷却 |
| 权限渐进 | 低风险读取与工作区写入可按能力向量渐进放开 |
| 审计 | 任务、Run、通知抑制、权限变更全链路可追溯 |
| 负反馈率 | 主动任务负反馈率持续低于 15% |

---

## 12. 关联架构决策

| ADR | 标题 | 要点 |
|-----|------|------|
| ADR-015 | Security 为最终边界 | 自主性不能越过安全上限 |
| ADR-020 | 自主性分阶段引入 | v0.2 只交付受控自主能力 |
| ADR-027 | 自主性集成在 clawx-runtime | 不新增独立 Crate |
| ADR-028 | ReAct 为基础推理模式 | 多步执行以 ReAct 为基础 |
| ADR-030 | 信任档案按 Agent 独立计算 | 且按能力向量细分 |
| ADR-032 | 主动任务采用 Task/Trigger/Run 模型 | 统一任务数据与恢复语义 |
| ADR-033 | service 为组合根 | runtime 通过 Port 与 scheduler/channel 交互 |
| ADR-034 | 主动通知必须经过 Attention Policy | 把“少打扰”做成系统能力 |
