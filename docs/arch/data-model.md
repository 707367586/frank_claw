# ClawX 数据模型与存储架构

**版本:** 3.3
**日期:** 2026年3月18日
**对应架构:** v4.2

---

## 1. 存储引擎概览

| 存储引擎 | 用途 | 数据类型 |
|---------|------|---------|
| **SQLite** | 主数据库 | Agent 配置、对话、记忆、任务、记忆变更审计 (memory_audit_log) |
| **Qdrant (embedded)** | 向量索引 | 文档 Embedding 向量；记忆 Embedding 向量 (v0.2+, ADR-011) |
| **Tantivy** | 全文索引 | BM25 倒排索引 |
| **文件系统** | 工作区/产物 | 用户文件、Agent 产物、版本点 |
| **JSONL 文件** | 安全审计日志 | SHA-256 哈希链审计记录 (`~/.clawx/audit/`, L12) |
| **macOS Keychain** | 密钥存储 | API Key、Token、加密密钥 |

> **审计系统分工说明：** ClawX 有两套独立的审计机制，服务于不同目标：
> - **安全审计日志** (JSONL + 哈希链)：记录所有 Agent 行为、工具调用、风险事件等安全相关操作，存储在 `~/.clawx/audit/`，以追加写入 + SHA-256 哈希链保证不可篡改（见 [security-architecture.md](./security-architecture.md) L12）。
> - **记忆变更审计** (SQLite `memory_audit_log` 表)：仅追踪 User Memory（共享记忆）的创建、修改、合并、删除操作，用于记忆来源追溯和冲突排查（见 [memory-architecture.md](./memory-architecture.md) §4.2）。
>
> 两者分开的原因：安全审计要求不可篡改（哈希链 + 追加写入），适合 JSONL；记忆审计需要结构化查询和关联（按 memory_id 查历史），适合 SQLite 关系表。

---

## 2. SQLite 核心数据模型

### 2.1 Agent 相关

```sql
-- Agent 配置
CREATE TABLE agents (
    id          TEXT PRIMARY KEY,           -- UUID v4
    name        TEXT NOT NULL,
    role        TEXT NOT NULL,              -- 角色描述
    system_prompt TEXT,                     -- System Prompt
    model_id    TEXT NOT NULL,              -- 绑定的 LLM 模型 ID
    icon        TEXT,                       -- 角色图标
    status      TEXT NOT NULL DEFAULT 'idle', -- idle/working/error/offline
    capabilities TEXT NOT NULL DEFAULT '[]', -- JSON: 启用的能力预设列表
    created_at  TEXT NOT NULL,              -- ISO 8601
    updated_at  TEXT NOT NULL,
    last_active_at TEXT
);

-- 对话
CREATE TABLE conversations (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id),
    title       TEXT,                       -- 自动生成或用户设定
    status      TEXT NOT NULL DEFAULT 'active', -- active/archived/deleted
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- 消息
CREATE TABLE messages (
    id          TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id),
    role        TEXT NOT NULL,              -- user/assistant/system/tool
    content     TEXT NOT NULL,
    metadata    TEXT,                       -- JSON: 附加信息 (tool_calls, citations, etc.)
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_messages_conversation ON messages(conversation_id, created_at);
```

### 2.2 记忆系统

> **权威定义见 [memory-architecture.md](./memory-architecture.md) §4.2**，此处保持同步。

```sql
-- 长期记忆 (与 memory-architecture.md §4.2 保持一致)
CREATE TABLE memories (
    id              TEXT PRIMARY KEY,            -- UUID v4
    scope           TEXT NOT NULL,               -- 'agent' | 'user'
    agent_id        TEXT,                        -- scope='agent' 时关联的 Agent ID
    kind            TEXT NOT NULL,               -- fact/preference/event/skill
    summary         TEXT NOT NULL,               -- 记忆摘要 (用于展示和快速匹配)
    content         TEXT NOT NULL,               -- JSON: 详细结构化内容
    importance      REAL NOT NULL DEFAULT 5.0,   -- 0-10 重要性评分
    freshness       REAL NOT NULL DEFAULT 1.0,   -- 0-1 鲜活度 (艾宾浩斯衰减)
    access_count    INTEGER NOT NULL DEFAULT 0,  -- 累计访问次数
    is_pinned       INTEGER NOT NULL DEFAULT 0,  -- 永久保留标记
    source_agent_id TEXT,                        -- 创建该记忆的 Agent ID
    source_type     TEXT NOT NULL DEFAULT 'implicit', -- implicit/explicit/consolidation
    superseded_by   TEXT,                        -- 被哪条记忆取代 (合并/更新时设置)
    qdrant_point_id TEXT,                        -- Qdrant 向量点 ID (v0.2+, v0.1 为 NULL)
    created_at      TEXT NOT NULL,               -- ISO 8601
    last_accessed_at TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX idx_memories_scope ON memories(scope, agent_id);
CREATE INDEX idx_memories_freshness ON memories(freshness) WHERE freshness > 0.05;
CREATE INDEX idx_memories_kind ON memories(kind);
CREATE INDEX idx_memories_active ON memories(scope, freshness)
    WHERE superseded_by IS NULL AND freshness > 0.05;

-- v0.1 全文检索索引 (ADR-011)
CREATE VIRTUAL TABLE memories_fts USING fts5(
    summary, content,
    content='memories', content_rowid='rowid'
);
```

> **更多记忆相关表**（权威定义见 [memory-architecture.md](./memory-architecture.md) §4.2）：
> - `memory_audit_log` — 共享记忆变更审计日志
> - `short_term_memories` — Session 级短期记忆（v0.2）
> - `memory_sessions` — Session 生命周期管理（v0.2）

### 2.3 知识库

```sql
-- 知识源文件夹
CREATE TABLE knowledge_sources (
    id          TEXT PRIMARY KEY,
    path        TEXT NOT NULL UNIQUE,       -- 监控的文件夹路径
    agent_id    TEXT,                       -- 关联 Agent (NULL = 全局)
    status      TEXT NOT NULL DEFAULT 'active', -- active/paused/error
    file_count  INTEGER NOT NULL DEFAULT 0,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    last_synced_at TEXT,
    created_at  TEXT NOT NULL
);

-- 文档索引
CREATE TABLE documents (
    id          TEXT PRIMARY KEY,
    source_id   TEXT NOT NULL REFERENCES knowledge_sources(id),
    file_path   TEXT NOT NULL,
    file_type   TEXT NOT NULL,              -- pdf/md/docx/jpg/mp3/...
    file_hash   TEXT NOT NULL,              -- SHA-256 用于增量更新
    file_size   INTEGER NOT NULL,
    chunk_count INTEGER NOT NULL DEFAULT 0,
    status      TEXT NOT NULL DEFAULT 'pending', -- pending/indexed/error
    indexed_at  TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_documents_source ON documents(source_id);
CREATE INDEX idx_documents_hash ON documents(file_hash);

-- 文档分块
CREATE TABLE chunks (
    id          TEXT PRIMARY KEY,
    document_id TEXT NOT NULL REFERENCES documents(id),
    chunk_index INTEGER NOT NULL,           -- 块序号
    content     TEXT NOT NULL,              -- 文本内容
    token_count INTEGER NOT NULL,
    qdrant_point_id TEXT,                   -- Qdrant 向量 ID
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_chunks_document ON chunks(document_id, chunk_index);
```

### 2.4 模型与 API 配置

```sql
-- LLM Provider 配置
CREATE TABLE llm_providers (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,              -- 显示名称
    type        TEXT NOT NULL,              -- openai/anthropic/ollama/custom
    base_url    TEXT NOT NULL,
    model_name  TEXT NOT NULL,              -- 模型标识 (gpt-4, claude-opus, etc.)
    parameters  TEXT NOT NULL DEFAULT '{}', -- JSON: temperature, max_tokens, etc.
    is_default  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
-- 注意：API Key 存储在 macOS Keychain，不在数据库中

-- 用量统计
CREATE TABLE usage_stats (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id),
    provider_id TEXT NOT NULL REFERENCES llm_providers(id),
    input_tokens INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    request_count INTEGER NOT NULL DEFAULT 0,
    date        TEXT NOT NULL,              -- YYYY-MM-DD
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_usage_date ON usage_stats(date);
CREATE INDEX idx_usage_agent ON usage_stats(agent_id, date);
```

### 2.5 主动任务系统 (v0.2)

> **权威定义见 [autonomy-architecture.md](./autonomy-architecture.md) §8**，此处与其保持一致。
> v0.2 对外开放 `time/event` 触发；`context/policy` 只保留枚举和扩展位，不作为默认开放能力。

```sql
-- 任务定义：描述“做什么”
CREATE TABLE tasks (
    id                TEXT PRIMARY KEY,
    agent_id          TEXT NOT NULL REFERENCES agents(id),
    name              TEXT NOT NULL,
    goal              TEXT NOT NULL,              -- 标准化后的执行目标
    source_kind       TEXT NOT NULL,              -- conversation/manual/suggestion/imported
    lifecycle_status  TEXT NOT NULL DEFAULT 'active', -- active/paused/archived
    default_max_steps INTEGER NOT NULL DEFAULT 10,
    default_timeout_secs INTEGER NOT NULL DEFAULT 1800,
    notification_policy TEXT NOT NULL DEFAULT '{}', -- JSON: quiet_hours/cooldown/digest 等
    suppression_state TEXT NOT NULL DEFAULT 'normal', -- normal/cooldown/paused_by_feedback
    last_run_at       TEXT,
    next_run_at       TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);
CREATE INDEX idx_tasks_agent_status ON tasks(agent_id, lifecycle_status);

-- 触发器：描述“何时做”
CREATE TABLE task_triggers (
    id                TEXT PRIMARY KEY,
    task_id           TEXT NOT NULL REFERENCES tasks(id),
    trigger_kind      TEXT NOT NULL,              -- time/event/context/policy
    trigger_config    TEXT NOT NULL,              -- JSON: cron/event filter/context rule/policy rule
    status            TEXT NOT NULL DEFAULT 'active', -- active/paused
    next_fire_at      TEXT,
    last_fired_at     TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);
CREATE INDEX idx_task_triggers_task ON task_triggers(task_id, status);
CREATE INDEX idx_task_triggers_next_fire ON task_triggers(next_fire_at) WHERE status = 'active';

-- Run：一次真正发生的执行实例
CREATE TABLE task_runs (
    id                TEXT PRIMARY KEY,
    task_id           TEXT NOT NULL REFERENCES tasks(id),
    trigger_id        TEXT REFERENCES task_triggers(id),
    idempotency_key   TEXT NOT NULL UNIQUE,
    run_status        TEXT NOT NULL,              -- queued/planning/running/waiting_confirmation/completed/failed/interrupted
    attempt           INTEGER NOT NULL DEFAULT 1,
    lease_owner       TEXT,                       -- 当前持有该 run 的 service 实例
    lease_expires_at  TEXT,
    checkpoint        TEXT NOT NULL DEFAULT '{}', -- JSON: steps/outputs/pending confirmation
    tokens_used       INTEGER NOT NULL DEFAULT 0,
    steps_count       INTEGER NOT NULL DEFAULT 0,
    result_summary    TEXT,
    failure_reason    TEXT,
    feedback_kind     TEXT,                       -- accepted/ignored/rejected/mute_forever/reduce_frequency
    feedback_reason   TEXT,
    notification_status TEXT NOT NULL DEFAULT 'pending', -- pending/sent/failed/suppressed
    triggered_at      TEXT NOT NULL,
    started_at        TEXT,
    finished_at       TEXT,
    created_at        TEXT NOT NULL
);
CREATE INDEX idx_task_runs_task_time ON task_runs(task_id, triggered_at DESC);
CREATE INDEX idx_task_runs_status ON task_runs(run_status, lease_expires_at);

-- 通知递送结果：记录是发了、失败了，还是被抑制了
CREATE TABLE task_notifications (
    id                TEXT PRIMARY KEY,
    run_id            TEXT NOT NULL REFERENCES task_runs(id),
    channel_kind      TEXT NOT NULL,              -- desktop/im/file
    target_ref        TEXT,                       -- channel id / file path / desktop
    delivery_status   TEXT NOT NULL,              -- pending/sent/failed/suppressed/digest_queued
    suppression_reason TEXT,
    payload_summary   TEXT,
    delivered_at      TEXT,
    created_at        TEXT NOT NULL
);
CREATE INDEX idx_task_notifications_run ON task_notifications(run_id, delivery_status);

-- 权限档案：按 Agent、按能力维度维护信任向量
CREATE TABLE permission_profiles (
    agent_id          TEXT PRIMARY KEY REFERENCES agents(id),
    capability_scores TEXT NOT NULL DEFAULT '{}', -- JSON: knowledge_read/workspace_write/external_send/memory_write/shell_exec
    safety_incidents  INTEGER NOT NULL DEFAULT 0,
    last_downgraded_at TEXT,
    created_at        TEXT NOT NULL,
    updated_at        TEXT NOT NULL
);

-- 权限变更审计
CREATE TABLE permission_events (
    id                TEXT PRIMARY KEY,
    agent_id          TEXT NOT NULL REFERENCES agents(id),
    capability        TEXT NOT NULL,
    old_level         TEXT NOT NULL,
    new_level         TEXT NOT NULL,
    reason            TEXT NOT NULL,
    run_id            TEXT REFERENCES task_runs(id),
    created_at        TEXT NOT NULL
);
CREATE INDEX idx_permission_events_agent_time ON permission_events(agent_id, created_at DESC);
```

### 2.6 工作区版本管理

> **存储位置:** vault 元数据存储在独立的 `~/.clawx/vault/index.db` 中，不在主数据库 `clawx.db` 中。独立存储便于 vault 目录整体迁移和独立备份。

```sql
-- 以下表位于 ~/.clawx/vault/index.db（独立 SQLite）

-- 版本点
CREATE TABLE vault_snapshots (
    id          TEXT PRIMARY KEY,
    label       TEXT NOT NULL UNIQUE,       -- clawx-{agent_id}-{task_id}-{timestamp}
    agent_id    TEXT,
    task_id     TEXT,
    description TEXT,                       -- 任务摘要
    disk_size   INTEGER NOT NULL DEFAULT 0, -- 占用磁盘空间 (bytes)
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_snapshots_created ON vault_snapshots(created_at);

-- 变更集
CREATE TABLE vault_changes (
    id          TEXT PRIMARY KEY,
    snapshot_id TEXT NOT NULL REFERENCES vault_snapshots(id),
    file_path   TEXT NOT NULL,
    change_type TEXT NOT NULL,              -- added/modified/deleted/renamed
    old_path    TEXT,                       -- renamed 时的原路径
    old_hash    TEXT,                       -- 变更前文件哈希
    new_hash    TEXT,                       -- 变更后文件哈希
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_changes_snapshot ON vault_changes(snapshot_id);
```

### 2.7 IM 渠道 (v0.2)

```sql
-- 渠道配置 (v0.2, clawx-channel)
CREATE TABLE channels (
    id          TEXT PRIMARY KEY,
    type        TEXT NOT NULL,              -- lark/telegram/slack/whatsapp/discord/wecom
    name        TEXT NOT NULL,
    config      TEXT NOT NULL,              -- JSON: 渠道专属配置 (token, webhook, etc.)
    agent_id    TEXT REFERENCES agents(id), -- 绑定的 Agent
    status      TEXT NOT NULL DEFAULT 'disconnected', -- connected/disconnected/error
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);
```

### 2.8 产物管理 (v0.3)

```sql
-- Agent 产物 (v0.3, clawx-artifact)
CREATE TABLE artifacts (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL REFERENCES agents(id),
    task_id     TEXT,
    file_name   TEXT NOT NULL,
    file_path   TEXT NOT NULL,              -- 相对于 workspace/artifacts/ 的路径
    file_type   TEXT NOT NULL,              -- pdf/py/html/png/...
    file_size   INTEGER NOT NULL,
    description TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_artifacts_agent ON artifacts(agent_id, created_at);
```

---

## 3. 向量存储 (Qdrant)

### 3.1 Collection 设计

```
Collection: "knowledge"
├── vector_size: 768 (nomic-embed-text) 或 512 (CLIP)
├── distance: Cosine
└── payload:
    ├── chunk_id: string
    ├── document_id: string
    ├── source_id: string
    ├── file_path: string
    ├── content_preview: string (前 200 字符)
    └── created_at: string

Collection: "memories" (v0.2+, ADR-011: v0.1 使用 SQLite FTS5)
├── vector_size: 768 (nomic-embed-text)
├── distance: Cosine
├── on_disk: true
└── payload:
    ├── memory_id: string (indexed)
    ├── scope: string (indexed, filterable)
    ├── agent_id: string (indexed, filterable)
    ├── kind: string (indexed, filterable)
    ├── summary: string
    ├── importance: float (indexed, filterable)
    ├── freshness: float (indexed, filterable)
    └── created_at: datetime
```

### 3.2 混合检索流程

```
用户查询
    │
    ├──────────────────┐
    ▼                  ▼
Qdrant               Tantivy
(向量语义搜索)       (BM25 关键词)
cos_similarity       tf-idf score
    │                  │
    └────────┬─────────┘
             ▼
     RRF 融合排序
     score = Σ 1/(k + rank_i)
     k = 60 (default)
             │
             ▼
     Top-N 结果返回
```

---

## 4. 文件系统存储

### 4.1 工作区结构

```
~/.clawx/workspace/
├── agents/
│   ├── {agent-id-1}/          # 每个 Agent 独立工作目录
│   │   ├── input/             # 用户拖入的文件副本
│   │   └── output/            # Agent 生成的产物
│   └── {agent-id-2}/
├── artifacts/                  # 全局产物归档
└── temp/                       # 临时文件 (定期清理)
```

### 4.2 版本点存储

```
~/.clawx/vault/
├── snapshots/
│   ├── {snapshot-id-1}/
│   │   ├── manifest.json      # 变更集清单
│   │   └── blobs/             # 变更前文件备份 (按 hash 存储)
│   └── {snapshot-id-2}/
└── index.db                   # SQLite (vault_snapshots + vault_changes)
```

---

## 5. 数据生命周期

### 5.1 记忆衰减

```
鲜活度 freshness = f(t, access_count, importance)

初始值: 1.0
衰减公式: freshness *= e^(-λ * days_since_last_access)
访问提升: freshness = min(1.0, freshness + 0.3)
归档阈值: freshness < 0.2
删除阈值: freshness < 0.05 且 未 pinned
```

### 5.2 版本点清理

| 时间范围 | 保留策略 |
|---------|---------|
| 0-7 天 | 全部保留 |
| 7-30 天 | 每天保留 1 个 |
| > 30 天 | 自动删除 |
| 磁盘 < 10% | 告警提醒 |

### 5.3 知识库增量更新

```
FSEvents 检测文件变更
    │
    ├── 新文件: 解析 → 分块 → Embedding → 写入索引
    ├── 修改文件: 比较 file_hash → 删除旧 chunks → 重新索引
    └── 删除文件: 删除关联 chunks + 向量点
```
