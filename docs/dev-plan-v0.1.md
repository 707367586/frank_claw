# ClawX V0.1 开发计划

> **核心原则：先搭架构骨架，再按垂直切片逐个实现功能**
> 每个 Phase 结束后 `cargo build --workspace` + `cargo test --workspace` 必须通过
> 代码与文档不一致时以文档为准

---

## Phase 1：工程治理与骨架清理 ✅

- [x] 删除已废弃 crate `clawx-daemon/`、`clawx-gateway/`
- [x] 从 workspace 移除对应条目
- [x] 新建 `clawx-controlplane-client` crate
- [x] 补充 workspace 依赖：qdrant-client、tantivy、clap、zeroize
- [x] 按 crate-dependency-graph.md 对齐所有 crate 的依赖
- [x] `cargo build` + `cargo clippy` 零 warning

---

## Phase 2：clawx-types 类型体系 ✅

- [x] memory.rs 按文档重写（MemoryScope/MemoryEntry/ScoredMemory 等）
- [x] traits.rs 定义 11 个核心 Port Trait
- [x] agent/llm/security/error 完善，新建 vault/knowledge/config/pagination
- [x] 28 个单元测试通过

---

## Phase 3：全链路架构骨架 ✅

> **目标：Service 能启动 → API 监听 → CLI 能连接 → 返回 stub 数据。整条管道通了但功能是空的。**

### 3.1 基础设施层 ✅

- [x] **clawx-config**：TOML 加载 + 默认值 + 首次创建 `~/.clawx/` 目录树
- [x] **clawx-eventbus**：EventBusPort trait + NoopEventBus + BroadcastEventBus 实现

### 3.2 数据库 ✅

- [x] SQLite WAL 模式 + 外键约束
- [x] 主数据库全部建表（agents/conversations/messages/memories/knowledge_sources/documents/chunks/llm_providers/usage_stats/memory_audit_log）
- [x] Vault 数据库建表（vault_snapshots/vault_changes）
- [x] FTS5 虚拟表（memories_fts）
- [x] 首次启动自动执行 migration
- [x] In-memory 测试支持

### 3.3 领域层 — trait stub 实现 ✅

- [x] **clawx-llm**：StubLlmProvider
- [x] **clawx-security**：PermissiveSecurityGuard
- [x] **clawx-vault**：StubVaultService
- [x] **clawx-memory**：StubMemoryService + StubWorkingMemoryManager
- [x] **clawx-kb**：StubKnowledgeService

### 3.4 Runtime 骨架 ✅

- [x] Runtime struct 持有所有 trait object
- [x] Agent 生命周期状态机（LifecycleManager）
- [x] Agent Loop 骨架（run_turn）
- [x] Dispatcher 消息路由

### 3.5 API 层 ✅

- [x] Axum Router 注册全部路由分组
- [x] control_token Bearer 认证中间件
- [x] UDS + TCP dev 模式

### 3.6 客户端与应用骨架 ✅

- [x] **clawx-controlplane-client**：HTTP 客户端骨架
- [x] **clawx-cli**：clap 子命令全部注册
- [x] **clawx-service**：Composition Root + 启动流程

---

## Phase 4 (F1)：Agent CRUD ✅

- [x] agent_repo.rs: create/get/list/update/delete/clone — 8 tests
- [x] API: 全部 Agent 端点真实处理器 + 错误响应
- [x] API 端到端测试 — 7 tests
- [x] CLI: agent 子命令注册

---

## Phase 5 (F2)：对话与 LLM 调用 ✅

- [x] conversation_repo.rs: create/get/list/delete + add_message/list_messages — 13 tests
- [x] clawx-llm: AnthropicProvider + OpenAiProvider 骨架
- [x] clawx-llm: LlmRouter 根据模型前缀选择 Provider
- [x] Runtime: Agent Loop（接收消息→组装上下文→调用 LLM→返回响应）
- [x] API: Conversations CRUD + Messages CRUD — 12 tests
- [x] API: `POST /conversations/:id/messages` SSE 流式响应（stream=true）
- [x] CLI: `clawx chat <agent-id>` 交互式对话 + SSE 流式输出

---

## Phase 6 (F3)：记忆系统 ✅

- [x] SqliteMemoryService（CRUD + FTS5 检索 + 衰减）— 17 tests
- [x] LlmMemoryExtractor（LLM prompt → MemoryCandidate 解析）— 10 tests
- [x] consolidation.rs（Jaccard 相似度去重 + 合并）— 10 tests
- [x] RealWorkingMemoryManager（系统提示词 + 记忆召回 + 对话历史 + Token 预算）— 6 tests
- [x] API: `/memories` 全部端点接入
- [x] Runtime: Agent Loop 集成记忆召回（注入 system prompt）+ 记忆提取（异步后台）

---

## Phase 7 (F4)：安全基线 ✅

- [x] L4 权限模型：CapabilityChecker
- [x] L5 DLP：regex 扫描（SSH Key/AWS Key/API Key/GitHub Token）
- [x] L6 网络白名单 + SSRF 防护 — 22 tests
- [x] L7 路径穿越防护
- [x] L11 GCRA 限速 — 9 tests
- [x] L12 哈希链审计 — 8 tests
- [x] ClawxSecurityGuard 整合实现

---

## Phase 8 (F5)：知识库引擎 ✅

- [x] parser（txt/md/csv/json 文本解析）
- [x] chunker（2048 字符分块 + 重叠 + 智能断点）
- [x] SqliteKnowledgeService（索引 + LIKE 检索 + 增量更新）— 17 tests
- [x] Tantivy BM25 全文检索 + RRF 混合检索融合 — 6 tests
- [x] FSEvents 文件监控（clawx-hal FsWatcher via notify crate）— 3 tests
- [ ] Qdrant 向量语义检索（需本地 Embedding 模型）— v0.2

---

## Phase 9 (F6)：工作区版本管理 ✅

- [x] SqliteVaultService（创建版本点 + 列表 + diff + rollback）— 8 tests
- [x] cleanup_old_snapshots（按天数清理）

---

## Phase 10：模型管理 + 系统运维 ✅

- [x] model_repo.rs: Provider CRUD — 9 tests
- [x] API: `/models` CRUD + `/system/health` + `/system/stats`
- [x] launchd plist 生成与安装（`clawx service install/uninstall/show`）

---

## Phase 11：Service 组装 + 真实实现接入 ✅

> **目标：用真实实现替换所有 stub，让 Service 成为可工作的完整系统**

- [x] clawx-service 组装真实实现：SqliteMemoryService、SqliteVaultService、SqliteKnowledgeService、LlmRouter 替换 stub
- [x] 真实 WorkingMemoryManager 实现 + 集成到 Runtime
- [x] Agent Loop 记忆集成：每轮对话后自动提取记忆并存储
- [x] SSE 流式消息：`POST /conversations/:id/messages` 支持 SSE
- [x] CLI 接入 Service：全部命令通过 controlplane-client 连接 service
- [x] launchd plist 生成（`clawx service install/uninstall/show`）

---

## Phase 12：集成测试与性能验收 ✅

### 12.1 端到端集成测试

- [x] Agent 完整生命周期：创建→更新→对话→Clone→删除
- [x] Model Provider CRUD 全流程
- [x] 对话隔离：Agent A 对话不可被 Agent B 读取
- [x] 安全：认证中间件拒绝无效/缺失 token
- [x] 错误处理：无效输入 400、不存在 404
- [x] 级联删除：删除对话自动清理消息
- [x] 记忆系统（真实 SQLite）：stats + search + 1000 条压力测试
- [x] 知识库完整流程：添加源→索引文件→检索→幂等重索引→删除源级联清理
- [x] Vault 完整流程：创建版本点→列表→获取→diff preview→回滚→404 处理
- [x] 安全基线：DLP 拦截 SSH Key/AWS Key、路径穿越阻断、网络白名单+SSRF 防护、Capability 权限模型、API 层路径安全检查

### 12.2 性能基线

- [x] 冷启动 < 2 秒（DB init + Runtime 构建）
- [x] 记忆召回 P50 < 50ms（100 条 FTS5 检索）
- [x] 记忆召回 P95 < 200ms（1000 条压力测试）
- [x] 知识库检索 P50 < 800ms（20 文件索引后搜索）
- [x] 1000 条记忆存储 + stats 验证（footprint baseline）

---

## Phase 13：V0.1 补齐 — 混合检索 + HAL + CLI Chat ✅

> **目标：补齐 PRD/架构对 V0.1 的完整要求，修复已知问题。**

### 13.1 修复集成测试 ✅

- [x] 修复 `security_dlp_blocks_sensitive_data` 测试（macOS `/tmp` → `/private/tmp` symlink 路径规范化）

### 13.2 知识库 Tantivy BM25 混合检索 ✅

- [x] `tantivy_index.rs`：TantivyIndex（BM25 全文索引/搜索/删除/提交）— 3 tests
- [x] `hybrid.rs`：RRF 融合排序（`score = Σ 1/(k + rank_i)`）— 3 tests
- [x] `SqliteKnowledgeService` 支持 `with_tantivy()` 构造，hybrid search 自动融合 BM25 + LIKE
- [x] `clawx-service` 组装 root 接入 TantivyIndex（路径 `~/.clawx/tantivy/`）

### 13.3 clawx-hal 基础实现 ✅

- [x] `fs_watcher.rs`：FSEvents 文件监控（via `notify` crate）— 3 tests
- [x] `keychain.rs`：macOS Keychain 凭证存储（set/get/delete via `security-framework`）— 2 tests
- [x] workspace 添加 `notify = "7"` + `security-framework = "3"` 依赖

### 13.4 CLI 交互式对话 ✅

- [x] `clawx chat <agent-id>` 交互式 REPL + SSE 流式输出
- [x] `ControlPlaneClient` 新增 `token()` / `http()` 方法
- [x] CLI 添加 `reqwest` + `futures` 依赖

---

## 当前进度总览

| Phase | 状态 | 测试数 |
|-------|------|--------|
| P1 工程治理 | ✅ | — |
| P2 类型体系 | ✅ | 28 |
| P3 架构骨架 | ✅ | 7 |
| P4 Agent CRUD | ✅ | 15 |
| P5 对话 + LLM | ✅ | 26 |
| P6 记忆系统 | ✅ | 43 |
| P7 安全基线 | ✅ | 39 |
| P8 知识库 | ✅ | 17 |
| P9 版本管理 | ✅ | 8 |
| P10 模型管理 | ✅ | 9 |
| P11 组装真实实现 | ✅ | — |
| P12 集成测试 + 性能验收 | ✅ | 23 |
| P13 V0.1 补齐 | ✅ | 11 |
| **合计** | | **329** |

---

## 风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| 本地 Embedding 性能不达标 | 先用云端 API 降级，本地选型并行评估 |
| Qdrant embedded 稳定性 | FTS5 作为检索降级，数据可从 SQLite 重建 |
| uniffi SwiftUI 兼容性 | 备选 swift-bridge 或手写 C FFI |
| 类型与文档不一致致返工 | Phase 2 已彻底对齐 |
