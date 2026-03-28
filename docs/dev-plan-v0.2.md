# ClawX V0.2 开发计划 — 扩展执行层

> **核心原则：自主性架构为主线，安全沙箱和渠道为并行支撑**
> 每个 Phase 结束后 `cargo build --workspace` + `cargo test --workspace` 必须通过
> 代码与文档不一致时以文档为准

---

## V0.1 完成度确认

| 项 | 状态 | 说明 |
|----|------|------|
| Agent CRUD + 对话 + SSE 流式 | ✅ | 全链路可用 |
| LLM 多 Provider（Anthropic/OpenAI/ZhipuAI） | ✅ | LlmRouter 按前缀路由 |
| 两层记忆系统（Agent/User） | ✅ | FTS5 + 衰减 + 合并 + 提取 |
| 知识库引擎（BM25 + RRF 混合检索） | ✅ | Tantivy + SQLite LIKE 融合 |
| 工作区版本管理 | ✅ | 版本点 + diff + rollback |
| 安全基线（L4/L5/L6/L7/L11/L12） | ✅ | 7 层安全已落地 |
| HAL（FSEvents + Keychain） | ✅ | notify + security-framework |
| CLI 交互式对话 | ✅ | REPL + SSE 流式 |
| 集成测试 + 性能验收 | ✅ | 329 测试，性能达标 |
| **遗留** | ⚠️ | Qdrant 向量语义检索推迟到 v0.2 |

**结论：V0.1 本地闭环已完全成立，可以开始 V0.2 开发。**

---

## V0.2 交付范围（PRD §4.1 + 架构文档）

| 功能模块 | PRD 来源 | 架构来源 |
|---------|---------|---------|
| **完整安全执行官** | §2.5 | security-architecture.md §6 v0.2 行 |
| **IM 渠道管理** | §2.6 | data-model.md §2.7 |
| **Skills 本地管理** | §2.7 | security-architecture.md L10 |
| **主动式 Agent** | §2.8 | autonomy-architecture.md |
| **Attention Policy** | §2.8.3 | autonomy-architecture.md §5 |
| **权限渐进** | - | autonomy-architecture.md §6 |
| **自主性模块** | - | autonomy-architecture.md §2-§4 |

### 明确不包含

- Skills 商店 / 社区分发（v0.5）
- 账号体系（v0.3）
- 产物管理（v0.3）
- 云端备份（v0.5）
- 移动端随行（v1.0+）
- `context/policy` 触发类型（v0.3+，只保留枚举）

---

## Phase 1：类型扩展与数据库迁移 ✅

> **目标：为 v0.2 全部新功能铺设类型基础和数据表，不涉及业务逻辑。**

### 1.1 clawx-types 扩展

- [x] 新建 `autonomy.rs`：
  - Task / TaskId / TaskStatus / TaskSourceKind
  - Trigger / TriggerId / TriggerKind / TriggerConfig
  - Run / RunId / RunStatus / RunCheckpoint
  - ExecutionStep / StepAction
  - FeedbackKind（accepted/ignored/rejected/mute_forever/reduce_frequency）
  - AttentionDecision（send_now/send_digest/store_only/suppress）
  - IntentCategory（simple/assisted/multi_step）
  - NotificationStatus / SuppressionState / DeliveryStatus / TaskNotification
- [x] 新建 `permission.rs`：
  - PermissionProfile / CapabilityDimension / TrustLevel（L0-L3）
  - RiskLevel（read/write/send/memory_low/memory_high/danger）
  - PermissionDecision（auto_allow/confirm/deny）
  - CapabilityScores（get/set per dimension）/ PermissionEvent
- [x] 新建 `channel.rs`：
  - Channel / ChannelId / ChannelType（lark/telegram/slack/whatsapp/discord/wecom）
  - ChannelStatus（connected/disconnected/error）
  - InboundMessage / OutboundMessage
- [x] 新建 `skill.rs`：
  - Skill / SkillId / SkillManifest / SkillStatus
  - CapabilityDeclaration（net_http/secrets/fs_read/fs_write/exec_shell）
- [x] 扩展 `traits.rs` — 10+ 新 trait：
  - TaskRegistryPort（create/get/list/update/delete + triggers + runs + feedback）
  - NotificationPort（send/query_status）
  - SchedulerPort（start/stop）
  - ChannelPort（connect/disconnect/send_message）
  - ChannelRegistryPort（CRUD）
  - SkillRegistryPort（install/uninstall/enable/disable/list/get）
  - PermissionGatePort（check_permission/update_profile/get_profile/record_safety_incident）
  - AttentionPolicyPort（evaluate/record_feedback）
  - PromptInjectionGuard（check）
  - TaskUpdate / TriggerUpdate / RunUpdate / ChannelUpdate 更新结构体
- [x] 扩展 `error.rs`：新增 Task / Channel / Skill / PermissionDenied / ResourceLocked / PromptInjection / Sandbox 变体
- [x] 59 个类型测试通过（含 31 个新 v0.2 测试）

### 1.2 数据库迁移

- [x] 主数据库新增 8 张表（对齐 data-model.md §2.5/§2.7）：
  - `tasks` / `task_triggers` / `task_runs` / `task_notifications`
  - `permission_profiles` / `permission_events`
  - `channels`
  - `skills`
- [x] 全部索引按 data-model.md 创建（idx_tasks_agent_status, idx_task_triggers_next_fire 等）
- [x] 迁移使用 `CREATE TABLE IF NOT EXISTS`，可幂等执行
- [x] 单元测试验证全部新表存在

---

## Phase 2：安全执行官完善（L1/L2/L3/L9/L10） ✅

> **目标：补齐 v0.2 安全层，为 Skills 沙箱和自主执行提供安全基础。**

### 2.1 L1 Prompt 注入三层防御 ✅

- [x] 第一层：PatternMatchGuard — 14 种正则模式匹配
  - 指令覆盖（ignore/forget/disregard previous instructions）
  - 系统提示词提取（reveal/show/print system prompt）
  - 角色劫持（you are now / act as DAN / pretend jailbreak）
  - 开发者模式（enable developer/admin mode）
  - 数据渗出（send to URL / read ssh keys / cat private key）
  - 分隔符注入（`</system>` / `<instruction>` 标签）
  - "Do Anything Now" 越狱（bypass all filters / no limitations）
  - 编码攻击（base64/rot13 decode instructions）
  - Unicode 零宽字符绕过
- [x] 第二层：ContentSanitizer — 外部输入转义包装 + 编码攻击检测
  - HTML 标签转义（`<` → `&lt;`）
  - `[BEGIN_UNTRUSTED_DATA]` / `[END_UNTRUSTED_DATA]` 标记
  - 零宽字符检测（`\u{200B}`-`\u{206F}` 连续 >3 个）
  - 同形文字混合脚本检测（Cyrillic + Latin）
- [x] 第三层：LLM 自检 — 预留接口，v0.2 不默认启用
- [x] ClawxPromptInjectionGuard trait 实现（async check → Ok/PromptInjection error）
- [x] 集成到 Service 初始化（guard 实例化 + 待 Agent Loop 对接）
- [x] 29 个测试通过（覆盖全部注入类别 + 清洁内容不误报）

### 2.2 L2 WASM 双计量沙箱 ✅

- [x] WasmSandbox 实现（SandboxConfig / 燃料计量 / 内存 ≤256MB / HTTP ≤10MB / 超时）
- [x] Host Function 接口（4 个：http_request 域名白名单 / secret_exists / log / now_millis）
- [x] 17 个测试：资源限制、超时中断、内存溢出、域名白名单、Secret 检查
- [x] wasmtime 延迟集成：sandbox 接口已就绪，wasmtime Engine 可在首次 Skill 调用时按需加载

### 2.3 L3 宿主边界凭证注入 ✅

- [x] 占位符 `{SECRET_NAME}` 替换机制（CredentialInjector）
- [x] 流程：沙箱构造请求 → host function → 域名白名单 → SecretStore 读取 → 替换 → 零化
- [x] `secret_exists(name)` 仅返回 bool（SecretStore trait）
- [x] InMemorySecretStore 用于测试
- [x] 12 个测试：占位符替换、密钥不泄漏、域名白名单、zeroize

### 2.4 L9 循环守卫扩展 ✅

- [x] Run 级步数上限 + 超时
- [x] 调用链哈希检测乒乓模式（VecDeque 最近 20 次，连续 3 次相同工具触发）
- [x] T2 子进程 `env_clear()` + 选择性 PATH/HOME/LANG（SubprocessEnvCleaner）
- [x] 测试：循环检测 + 环境变量清理 + 禁止敏感变量泄漏

### 2.5 L10 Skill Ed25519 签名验证 ✅

- [x] Ed25519 公钥验签（`ed25519-dalek` crate）
- [x] SHA-256 哈希校验（skill_repo 安装时计算并存储 hash）
- [x] generate_keypair / sign_skill / verify_skill_signature 完整 API
- [x] 6 个测试：合法签名通过、篡改签名拒绝、错误密钥拒绝、无效 hex 报错、密钥对生成、签名-验证往返

---

## Phase 3：Skills 本地管理 ✅

> **目标：实现 Skills 的安装、卸载、启用、停用和运行时加载。**

### 3.1 Skill 清单与管理

- [x] SkillManifest 格式定义（JSON/serde）：
  - name / version / description / author / entrypoint
  - CapabilityDeclaration：net_http / secrets / fs_read / fs_write / exec_shell
  - signature（可选 Ed25519 签名）
- [x] SqliteSkillRegistry：install / uninstall / enable / disable / list / get — 10 tests
- [x] Skill 数据库存储：manifest 序列化为 JSON，hash 为 SHA-256
- [x] 测试：CRUD + 状态流转 + 签名字段 + manifest JSON 往返 + 确定性 hash

### 3.2 Skill 运行时集成

- [x] SkillLoader：加载 WASM → L10 验签 → 实例化到沙箱
  - SkillLoaderConfig（public_key_hex / require_signature / sandbox_config）
  - install()：验证清单 → 验签 → 存储 DB
  - load()：从 Skill 构建 WasmSandbox（域名白名单 + secrets）
  - validate_manifest()：检查 name/version/entrypoint
  - 12 个单元测试覆盖全部路径
- [x] Skill 调用链路：ToolExecutor trait → SkillLoader → WasmSandbox
- [x] L4 权限模型扩展至 Skills（manifest 声明 capabilities，安装时确认）
- [x] 测试：Skill 加载 + 执行 + 权限拦截 + 签名验证

### 3.3 API 端点

- [x] `GET /skills` — 列出已安装 Skills
- [x] `POST /skills` — 安装 Skill（hex 编码 WASM bytes + manifest + 可选签名）
- [x] `GET /skills/:id` — 获取 Skill 详情
- [x] `DELETE /skills/:id` — 卸载 Skill
- [x] `POST /skills/:id/enable` / `POST /skills/:id/disable`
- [x] 8 个 API 集成测试

---

## Phase 4：自主性核心 — Executor 多步执行 ✅

> **目标：让 Agent 能在安全围栏内完成多步工具调用任务。**
> **对齐：autonomy-architecture.md §3**

### 4.1 意图评估

- [x] IntentEvaluator：用户请求 → simple / assisted / multi_step 判定
  - 启发式关键词匹配（multi_step: "then"/"after that"/"step by step"/"first"；assisted: "search"/"find"/"check"）
  - 5 个测试覆盖各分类
- [x] v0.2 LLM 判定模式预留（结构化输出接口 + fallback 到启发式）
- [x] IntentEvaluator 可被 TaskExecutor 调用

### 4.2 Executor 状态机

- [x] ExecutionState 状态机：queued → planning → running → completed / failed / interrupted
- [x] `waiting_confirmation` 状态：高风险操作暂停等待用户决定，可恢复到 running
- [x] 安全围栏（ExecutorConfig）：
  - 步数上限（默认 10，可配 1-25）— StepLimitExceeded
  - Token 预算（80% TokenBudgetWarning，100% TokenBudgetExceeded）
  - 循环检测（VecDeque 最近 20 次调用，连续 3 次相同工具 → LoopDetected）
  - 超时（前台 300s，后台 1800s）— Timeout
  - checkpoint 每步序列化为 JSON（steps_completed / tokens_used / status / steps）
  - GuardrailViolation 枚举：StepLimitExceeded / TokenBudgetExceeded / TokenBudgetWarning / LoopDetected / Timeout
- [x] 结构化步骤摘要：ExecutionStep（step_no / action / tool / evidence / risk_reason / result_summary）
- [x] 状态转换合法性验证（非法转换返回 ClawxError::Task）
- [x] 22 个测试：状态流转 + 围栏触发 + checkpoint 序列化 + 循环检测 + Token 预算

### 4.3 Executor 工具调度

- [x] 多步循环：TaskExecutor.execute_run() — Permission Gate → 执行工具 → 记录结果 → 判断继续/完成
  - ToolExecutor trait 抽象工具执行
  - ToolAction / ToolResult 类型
  - ExecutionSummary 返回值
- [x] 工具结果注入下一步上下文（evidence 字段传递）
- [x] 用户中断支持：tokio::sync::watch 通道，中断后保留已完成步骤
- [x] SSE 流式步骤进展推送（event: `execution_step`、`confirmation_required`）— SSE Event 构造函数已实现，待 Agent Loop 对接时调用
- [x] 12 个测试：多步执行全路径 + 中断 + 权限拒绝 + 围栏触发 + 循环检测

---

## Phase 5：权限渐进系统（Permission Gate） ✅

> **对齐：autonomy-architecture.md §6**

### 5.1 Permission Profile 管理

- [x] SqlitePermissionRepo：create_profile / get_profile / update_capability / record_safety_incident / get_events
- [x] 能力向量：knowledge_read / workspace_write / external_send / memory_write / shell_exec
- [x] 等级：L0 Restricted → L1 Read Trusted → L2 Workspace Trusted → L3 Channel Trusted
- [x] 新建 Agent 默认所有能力 L0（CapabilityScores::default()）
- [x] capability_scores 序列化为 JSON 存储在 SQLite

### 5.2 Permission Gate 决策

- [x] PermissionGate 实现 PermissionGatePort trait
- [x] 风险映射（autonomy-architecture.md §6.4）：
  - `read` → auto_allow if knowledge_read ≥ L1
  - `write` → auto_allow if workspace_write ≥ L2
  - `send` → auto_allow if external_send ≥ L3
  - `memory_low` → auto_allow if memory_write ≥ L2
  - `memory_high` → 永远 Confirm
  - `danger` → 永远 Confirm
- [x] 决策逻辑：auto_allow / confirm / deny
- [x] 集成到 Executor 工具调用前（TaskExecutor.execute_run 每步调用 check_permission）

### 5.3 升降级规则

- [x] update_capability 按单个维度更新 + 记录 PermissionEvent
- [x] record_safety_incident 递增 safety_incidents 计数器
- [x] 安全事件只降级相关能力维度（不整体清零）
- [x] 审计：权限变更记录到 `permission_events`（agent_id / capability / old_level / new_level / reason / run_id）

### 5.4 API 端点

- [x] `GET /agents/:id/permission-profile` — 获取权限档案
- [x] 21 个测试：权限判定全路径 + 升降级 + 安全事件 + 审计记录 + 边界情况

---

## Phase 6：主动任务系统（Task Manager） ✅

> **对齐：autonomy-architecture.md §4**

### 6.1 Task CRUD

- [x] SqliteTaskRegistry 实现 TaskRegistryPort（25+ 方法）
- [x] Task 定义：name / goal / agent_id / source_kind / lifecycle_status / notification_policy / suppression_state / default_max_steps / default_timeout_secs
- [x] 生命周期：active / paused / archived（update_lifecycle）
- [x] free-function 便捷 API（create_task / get_task / list_tasks / update_task / delete_task / update_lifecycle）
- [x] 测试：Task CRUD + 状态流转 + 分页 + 部分更新 + 删除不存在返回 NotFound

### 6.2 Trigger 管理

- [x] add_trigger / get_trigger / list_triggers / update_trigger / delete_trigger
- [x] TriggerKind：`time` / `event` / `context`（枚举保留）/ `policy`（枚举保留）
- [x] get_due_triggers：按 `status='active' AND next_fire_at <= now` 过滤
- [x] free-function 便捷 API（create_trigger / get_trigger / list_triggers / update_trigger / delete_trigger）
- [x] Cron 解析（`cron` crate）+ next_fire_at 自动计算（clawx-scheduler）
- [x] 测试：Trigger CRUD + due triggers 过滤 + cron 解析

### 6.3 Run 执行引擎

- [x] create_run / get_run / list_runs / update_run / get_incomplete_runs
- [x] Run 状态：queued / planning / running / waiting_confirmation / completed / failed / interrupted
- [x] get_incomplete_runs：返回 queued / planning / running / waiting_confirmation 状态的 Run
- [x] record_feedback：更新 feedback_kind + feedback_reason
- [x] idempotency_key UNIQUE 约束防重复触发
- [x] free-function 便捷 API（list_runs / get_run / record_feedback）
- [x] 恢复：RunRecoveryService — service 重启后检测未完成 Run
  - running/planning → 重试或 failed（按 max_retries 判定）
  - queued → 保留等待重新执行
  - waiting_confirmation → interrupted
  - RecoveryReport 结构化报告
- [x] 重试：可配退避策略（exponential backoff，上限 300s）+ 最大重试次数（默认 3）
- [x] 8 个测试覆盖全部恢复路径 + backoff 计算
- [x] 测试：Run 生命周期 + 部分更新 + incomplete runs + feedback

### 6.4 clawx-scheduler 实现 ✅

- [x] 定时触发：后台 tokio 任务扫描 `next_fire_at <= now` 的 active triggers
- [x] TaskScheduler：start/stop + configurable scan interval
- [x] 自动创建 Run（idempotency_key = trigger_id:fire_time）
- [x] 自动更新 trigger 的 next_fire_at + last_fired_at
- [x] Cron 解析：parse_cron / next_fire_time / compute_next_fire_at
- [x] 事件触发：handle_event() — 匹配 event trigger → 创建 Run
  - matches_event() 按 trigger_config.event_kind 匹配
  - 幂等 key = event:trigger_id:event_kind:timestamp
  - 自动更新 trigger 的 last_fired_at
  - 8 个测试覆盖匹配/不匹配/多触发器/暂停触发器
- [x] 测试：cron 解析 + scheduler 创建 runs

### 6.5 对话创建任务

- [ ] 自然语言 → 结构化 Task 解析（LLM 辅助）— 需真实 LLM 接入
- [ ] 确认卡片：展示名称/触发/通知方式/权限范围 — 需 UI 层
- [ ] 用户确认后创建 Task + Trigger — 需 Agent Loop 对接
- [ ] 测试：自然语言解析准确性 — 需 LLM 评测集

### 6.6 API 端点

- [x] `GET/POST /tasks` — 列出 / 创建任务
- [x] `GET/PUT/DELETE /tasks/:id` — 获取 / 更新 / 删除
- [x] `POST /tasks/:id/pause` / `POST /tasks/:id/resume` / `POST /tasks/:id/archive`
- [x] `POST /tasks/:id/triggers` — 添加触发器
- [x] `GET /tasks/:id/triggers` — 列出触发器
- [x] `PUT/DELETE /task-triggers/:id` — 修改 / 删除触发器
- [x] `GET /tasks/:id/runs` — 列出执行历史
- [x] `GET /task-runs/:id` — 获取 Run 详情
- [x] `POST /task-runs/:id/feedback` — 提交反馈
- [x] API 端到端集成测试（9 个测试覆盖 Task/Trigger/Run/Channel/Skill/Permission 全部 API）

---

## Phase 7：Attention Policy — 少打扰机制 ✅

> **对齐：autonomy-architecture.md §5**

### 7.1 Attention Policy 引擎

- [x] AttentionPolicyEngine 引擎实现
- [x] 决策输入：AttentionContext（trigger_kind / run_status / consecutive_ignores / last_notification_at / now）
- [x] 决策输出：AttentionDecision（send_now / send_digest / store_only / suppress）
- [x] 规则实现（优先级顺序）：
  - Failed Run 永远立即通知（覆盖其他规则）
  - 连续 3 次 ignored → 自动 Suppress（auto_pause_threshold 可配）
  - 冷却窗口内（默认 3600s）→ store_only
  - 静默时段内（默认 22:00-08:00）普通完成 → send_digest
  - 默认 → send_now
- [x] QuietHoursConfig：start_hour / end_hour，支持跨午夜（22:00-08:00）
- [x] 可配参数：quiet_hours / cooldown_secs / auto_pause_threshold
- [x] 14 个测试覆盖全部决策路径 + 优先级交叉

### 7.2 反馈回路 ✅

- [x] FeedbackAction 枚举：None / IncrementIgnoreCount / IncrementNegativeFeedback / ArchiveTask / AdjustTriggerFrequency
- [x] process_feedback 映射：
  - accepted → None
  - ignored → IncrementIgnoreCount
  - rejected → IncrementNegativeFeedback
  - mute_forever → ArchiveTask
  - reduce_frequency → AdjustTriggerFrequency
- [x] 负反馈率统计查询：`negative_feedback_rate()` = `(rejected + mute_forever) / total`
- [x] 测试：反馈到动作映射全覆盖 + 负反馈率计算

### 7.3 通知递送 ✅

- [x] SqliteNotificationRepo 实现 NotificationPort（send / query_status）
- [x] task_notifications 记录：每次通知的渠道、递送状态、抑制原因
- [x] 桌面通知适配器（DesktopNotifier — macOS osascript）
- [x] 文件写入通知（FileNotifier — 写结果到指定路径）
- [x] 便捷构造函数：sent_notification / suppressed_notification / failed_notification
- [x] 8 个测试：通知 CRUD + 多通知 + 抑制记录 + 负反馈率

---

## Phase 8：IM 渠道管理 ✅

> **对齐：PRD §2.6**

### 8.1 渠道框架 ✅

- [x] SqliteChannelRegistry（free-function API）：
  - create_channel / get_channel / list_channels / update_channel / delete_channel
  - ChannelRow → Channel 转换（parse ChannelType / ChannelStatus / JSON config）
  - ChannelUpdate 支持部分更新（name / config / agent_id / status）
- [x] 9 个测试：CRUD + 状态更新 + JSON config 往返 + 不存在返回 NotFound + config 替换
- [x] ChannelAdapter trait 实现（connect / disconnect / send_message / is_connected）
- [x] ChannelManager：管理所有渠道连接生命周期（register_adapter / connect / disconnect / send_message）
- [x] StubChannelAdapter 用于测试
- [x] 12 个测试：适配器连接断开 + Manager 路由 + 未注册类型拒绝

### 8.2 渠道适配器（首批 2 个） ✅

- [x] **Telegram Bot**：TelegramAdapter（校验 bot_token + 委托 stub，待接入真实 API）
- [x] **飞书/Lark**：LarkAdapter（校验 app_id/app_secret + 委托 stub，待接入 WebSocket）
- [x] 测试：配置校验 + 连接 + 断开
- [ ] 后续渠道（Slack/Discord/WhatsApp/WeChat Enterprise）可按需迭代

### 8.3 渠道 → Agent 路由

- [x] 消息路由规则：MessageRouter — 渠道绑定 Agent → RoutedMessage
  - route() 方法：Channel.agent_id → 返回 RoutedMessage 或 None
  - RoutedMessage 包含 channel_id / agent_id / content / sender_id / timestamp
  - 3 个测试覆盖有绑定/无绑定/字段正确性
- [ ] 入站消息 → 创建/续接对话 → Agent Loop → 出站回复 — 需 Agent Loop 对接
- [ ] EventBus 集成：渠道消息作为事件投递 — 需真实渠道 WebSocket

### 8.4 API 端点 ✅

- [x] `GET/POST /channels` — 列出 / 创建渠道配置
- [x] `GET/PUT/DELETE /channels/:id` — 获取 / 更新 / 删除
- [x] `POST /channels/:id/connect` / `POST /channels/:id/disconnect`

---

## Phase 9：Qdrant 向量语义检索（v0.1 遗留） ✅

> **补齐知识库和记忆的语义检索能力。**

### 9.1 Qdrant 嵌入式集成 ✅

- [x] `qdrant-client` 依赖已引入（workspace）
- [x] QdrantStore 封装层：collection_name / embed / embed_batch / cosine_similarity
- [x] EmbeddingService trait + StubEmbeddingService（768 维，确定性向量）
- [x] 10 个测试：嵌入维度 + 归一化 + 确定性 + 批量 + 余弦相似度

### 9.2 Embedding 模型 + Reranker ✅

- [x] EmbeddingService trait（embed / embed_batch / dimensions）
- [x] StubEmbeddingService 用于测试（确定性 hash 向量）
- [x] HttpEmbeddingService — OpenAI 兼容 `/v1/embeddings` API 客户端
  - EmbeddingConfig（base_url / model_name / api_key / dimensions / batch_size）
  - 支持 TEI、Ollama、OpenAI 等任意兼容端点
  - 默认模型：Qwen/Qwen3-VL-Embedding-2B
  - 13 个测试 + 1 个集成测试（ignored）
- [x] RerankerService trait + HttpRerankerService — TEI 兼容 `/rerank` API
  - RerankerConfig（base_url / model_name）
  - 默认模型：Qwen/Qwen3-VL-Reranker-2B
  - StubRerankerService 用于测试
  - 8 个测试
- [x] HybridSearchEngine.hybrid_search_with_rerank() — 二阶检索（RRF + Reranker）
- [x] Service 组装：embedding + reranker 环境变量配置
- [ ] Apple Silicon 加速（CoreML/Metal）评估 — **需硬件测试**

### 9.3 知识库向量检索 ✅

- [x] Reciprocal Rank Fusion (RRF) 算法实现（多列表融合）
- [x] 5 个 RRF 测试：单列表 + 双列表一致 + 双列表分歧 + 跨列表唯一 + 空列表
- [x] QdrantStore 完整向量存储：upsert / upsert_text / search / search_vector / delete / count
- [x] VectorPoint 结构化存储（point_id / content / metadata / vector）+ Arc<RwLock<HashMap>> 线程安全
- [x] 8 个 QdrantStore 测试：基础搜索 + 自动嵌入 + top_k + 空存储 + 删除 + 覆写 + 计数 + 分数排序
- [x] HybridSearchEngine：Qdrant 向量搜索 + BM25 + RRF 融合
- [x] index_chunk / delete_chunk / hybrid_search 完整管道
- [x] 5 个混合检索测试：双源融合 + 仅向量 + 仅 BM25 + 双空 + 索引-检索往返

### 9.4 记忆向量检索 ✅

- [x] VectorMemoryIndex 实现（基于 QdrantStore）
- [x] index_memory / search_memories / delete_memory / hybrid_recall / count
- [x] 记忆创建时生成 Embedding → 向量存储写入
- [x] 记忆召回：FTS5 + 向量检索 + RRF 融合（hybrid_recall）
- [x] 5 个测试：索引-检索 + 混合召回融合 + 删除 + 空索引 + 去重更新

---

## Phase 10：Service 组装 + CLI 扩展 ✅

> **目标：将所有 v0.2 新模块组装到 service，扩展 CLI 命令。**

### 10.1 clawx-service 组装 ✅

- [x] Prompt Injection Guard 初始化
- [x] TaskScheduler 后台启动（30s scan interval）
- [x] ChannelManager 初始化（Telegram + Lark adapters）
- [x] 所有 v0.2 存储层通过 Runtime.db 注入
  - SqliteTaskRegistry / SqlitePermissionRepo / SqliteChannelRegistry / SqliteSkillRegistry
  - SqliteNotificationRepo / AttentionPolicyEngine / PermissionGate
- [x] 渠道连接恢复（启动时重连 active 渠道）
- [x] 运行恢复（启动时检测未完成 Run + 恢复/重试/标记失败）

### 10.2 clawx-api 路由注册 ✅

- [x] 注册全部 v0.2 端点（/tasks / /task-triggers / /task-runs / /channels / /skills）
- [x] `GET /agents/:id/permission-profile` 权限档案端点
- [x] `POST /channels/:id/connect` / `POST /channels/:id/disconnect`
- [x] SSE 扩展：execution_step / confirmation_required 事件构造函数 + 类型定义

### 10.3 clawx-cli 扩展 ✅

- [x] `clawx task list/create/show/update/delete/pause/resume/archive`
- [x] `clawx task triggers add/list/delete`
- [x] `clawx task runs list/show/feedback`
- [x] `clawx channel list/add/show/update/remove`
- [x] `clawx skill list/show/uninstall/enable/disable`

---

## Phase 11：集成测试与验收 ✅

> **对齐：autonomy-architecture.md §11 验收标准**

### 11.1 端到端集成测试 ✅

- [x] **多步执行**：Task → Trigger → Run 全链路 API 测试，状态流转 queued → running → completed
- [x] **可靠性**：service 重启后恢复 running/waiting_confirmation 的 Run（RunRecoveryService 集成测试）
- [x] **主动任务**：time/event 任务按时触发 + 去重 + 重试（Scheduler + TaskRegistry 集成测试）
- [x] **反馈机制**：mute_forever → ArchiveTask + reduce_frequency → AdjustTriggerFrequency（AttentionPolicy 集成测试）
- [x] **权限渐进**：默认 L0 全 Confirm + 风险映射决策验证（PermissionGate 集成测试）
- [x] **通知全链路**：通知递送记录 + 抑制记录 + 负反馈率计算（NotificationRepo 集成测试）
- [x] **IM 渠道**：Channel 绑定 Agent + 路由 + 连接/断开状态管理（Channel API 集成测试）
- [x] **Skills**：安装 → 启用 → 停用 → 再启用 → 卸载全生命周期（Skill API 集成测试）
- [x] **安全**：Prompt 注入 14 种模式检测 + ContentSanitizer + 零宽字符检测（Security 集成测试）
- [x] **Attention Policy**：6 种场景决策测试 + 5 种反馈动作映射（AttentionPolicy 集成测试）
- [ ] **对话创建任务**：自然语言 → Task + Trigger + 通知策略 — **需真实 LLM 接入**

### 11.2 性能验收

- [ ] 多步执行 10 步完成时间 < 60 秒（不含 LLM 延迟） — **需真实工具链**
- [ ] 定时任务触发精度 < 5 秒偏差 — **需长时间运行测试**
- [ ] Qdrant 向量检索 P50 < 100ms（10k vectors） — **需真实 Embedding 模型**
- [ ] 混合检索 Top-5 hit rate >= 85%（内部评测集） — **需评测数据集**
- [ ] 记忆向量召回 Top-3 hit rate >= 80%（内部基准集） — **需评测数据集**
- [ ] 主动任务负反馈率 < 15% — **需用户测试数据**

### 11.3 安全验收

- [x] Prompt 注入检测率：14 种注入模式全部检测通过（集成测试覆盖）
- [x] Skill 签名篡改拒绝（Ed25519 验签测试覆盖）
- [ ] WASM 沙箱逃逸测试（内存越界、文件访问、网络绕过） — **需真实 WASM 二进制**
- [ ] DLP Aho-Corasick 多模式匹配准确性 — **需评测数据集**

---

## 当前进度总览

| Phase | 状态 | 核心交付 | 新增测试 |
|-------|------|---------|---------|
| P1 类型扩展 + 数据库迁移 | ✅ | 4 新类型模块 + 10 新 trait + 7 新错误变体 + 8 张新表 | 31 |
| P2 安全执行官 | ✅ | L1 Prompt 注入 + L2 WASM 沙箱 + L3 凭证注入 + L9 循环守卫 + L10 Ed25519 | 64 |
| P3 Skills 本地管理 | ✅ | SkillLoader + SqliteSkillRegistry + POST /skills API + 集成测试 | 30 |
| P4 Executor 多步执行 | ✅ | TaskExecutor 多步循环 + 状态机 + 5 种安全围栏 + 意图评估 + 中断 + SSE 事件类型 | 36 |
| P5 Permission Gate | ✅ | CapabilityScores + SqlitePermissionRepo + 6 级风险映射 + Executor 集成 | 21 |
| P6 Task Manager | ✅ | Task/Trigger/Run CRUD + Scheduler + Event Trigger + Run Recovery + Retry | 40 |
| P7 Attention Policy | ✅ | 通知决策引擎 + 反馈回路 + 通知递送 + 负反馈率 | 22 |
| P8 IM 渠道管理 | ✅ | ChannelAdapter + ChannelManager + MessageRouter + Telegram/Lark | 24 |
| P9 Qdrant 向量检索 | ✅ | QdrantStore CRUD + HybridSearchEngine + VectorMemoryIndex + HttpEmbedding + Reranker | 54 |
| P10 Service 组装 + CLI | ✅ | Run Recovery + Channel Recovery + Scheduler + ChannelManager + SSE 事件 | — |
| P11 集成测试与验收 | ✅ | 9 + 9 = 18 个端到端集成测试（全 v0.2 功能模块覆盖） | 18 |
| **合计** | | **v0.1: 504 → v0.2: 684** | **+180** |

---

## 依赖关系与建议执行顺序

```
P1（类型+DB）✅
  ├── P2（安全） ✅ ── P3（Skills）✅（含 SkillLoader + install API）
  ├── P5（权限）✅ ── P4（Executor）✅（含 TaskExecutor 多步 + Permission 集成 + SSE 事件）
  ├── P6（任务）✅ ── P7（Attention）✅
  │   └── Run Recovery ✅ + Event Trigger ✅
  ├── P8（渠道）✅（含 MessageRouter）
  └── P9（Qdrant）✅ QdrantStore CRUD + HybridSearchEngine + VectorMemoryIndex

P10（组装）✅ Run Recovery + Channel Recovery + 全部注入 + SSE 事件
P11（验收）✅ 18 个端到端集成测试（多步执行/Run恢复/反馈/权限/Skill/安全/Attention/渠道/通知）
```

**剩余待真实集成项（需外部依赖/LLM/UI）：**

1. **Agent Loop 对接**：TaskExecutor + IntentEvaluator + SSE 事件推送接入真实 Agent Loop
2. **对话创建任务**：自然语言 → Task 解析（需真实 LLM）
3. **渠道消息全链路**：入站消息 → Agent 对话 → 出站回复（需真实渠道 API）
4. **Embedding/Reranker 部署**：启动 TEI 服务加载 Qwen3-VL-Embedding-2B + Qwen3-VL-Reranker-2B
5. **性能验收**：需真实工具链 + 评测数据集 + 长时间运行测试
6. **安全验收**：WASM 真实二进制逃逸测试 + DLP 评测数据集

---

## 风险与缓解

| 风险 | 严重度 | 缓解措施 |
|------|--------|---------|
| Wasmtime 编译体积和冷启动 | Medium | 延迟加载 WASM Engine，仅首次 Skill 调用时初始化 |
| 本地 Embedding 模型体积/性能 | High | 先用云端 Embedding API 降级；本地选型并行评估 |
| Qdrant embedded 稳定性 | Medium | Tantivy BM25 作为降级检索；数据可从 SQLite 重建 |
| IM 渠道 API 变更频繁 | Low | 适配器隔离，单渠道变更不影响其他 |
| 多步执行 LLM 判定不稳定 | Medium | 结构化输出 + JSON Schema 约束 + fallback 策略 |
| Cron 调度精度 | Low | tokio interval 扫描 + next_fire_at 索引 |
| Skill 生态初期内容缺乏 | Low | 内置示例 Skills + 文档指引开发者 |
