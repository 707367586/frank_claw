# ClawX V0.2 剩余开发执行计划

> **原则：每完成一步打勾，cargo build + cargo test 必须通过**
> **日期：2026-03-28**

---

## Phase A：Agent Loop ↔ TaskExecutor 对接（核心闭环）

- [x] A1. Agent Loop 增加意图判定：调用 IntentEvaluator，multi_step 时委托 TaskExecutor
- [x] A2. Runtime 扩展：注入 TaskRegistry + PermissionGate + TaskExecutor 依赖
- [x] A3. SSE 流式对接：execution_step / confirmation_required 事件激活（去掉 dead_code）
- [x] A4. 用户确认/中断 API：POST /task-runs/:id/confirm + POST /task-runs/:id/interrupt
- [x] A5. 端到端集成测试：对话 → 意图判定 → 多步执行 → 完成

## Phase B：对话创建任务（NLP → Task）

- [x] B1. 设计 LLM 结构化输出 schema（Task name/goal/trigger/notification）
- [x] B2. TaskCreationParser：LLM 调用 → JSON 解析 → Task + Trigger 创建
- [x] B3. Agent Loop 集成：检测"创建任务"意图 → Parser → 确认流程
- [x] B4. 测试：多种自然语言表述正确解析（12 个测试）

## Phase C：渠道消息全链路

- [x] C1. 入站消息 → MessageRouter → 创建/续接 Conversation
- [x] C2. Agent Loop 处理渠道消息 → 生成回复 → 出站到渠道
- [x] C3. EventBus 接入：渠道消息 → EventBus → 触发 event triggers
- [ ] C4. Telegram/Lark 真实 API 替换 stub

## Phase D：Tauri GUI

- [x] D1. Tauri v2 + React + TS 工程初始化
- [x] D2. 侧边栏 + Agent 列表 + 对话视图
- [ ] D3. 知识库管理页
- [ ] D4. 定时任务管理页
- [ ] D5. Connectors（渠道）管理页
- [ ] D6. 设置页（LLM 配置、安全、通知偏好）
- [ ] D7. SSE 对接：流式对话 + 执行步骤实时展示

## Phase E：验收与优化

- [ ] E1. 性能基准测试框架 + 6 项指标
- [ ] E2. WASM 沙箱逃逸测试
- [ ] E3. DLP Aho-Corasick 升级（可选）
- [ ] E4. Apple Silicon GPU 加速评估（可选）
