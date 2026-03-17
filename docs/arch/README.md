# ClawX 架构文档索引

**对应 PRD:** v2.0 | **架构版本:** v4.0 | **日期:** 2026-03-18

**v4.0 变更说明:** 基于 IronClaw/OpenFang 等项目安全实践，安全架构从 6 层升级为 12 层纵深防御体系；新增智能模型路由、MCP 协议支持、宿主边界凭证注入等关键架构决策。

---

## 文档目录

| 文档 | 内容 | 阅读顺序 |
|------|------|---------|
| [architecture-v3.0.md](./architecture-v3.0.md) | **系统总体架构 (v4.0)** — 分层架构、模块职责、核心数据流、部署架构、技术栈选型、智能模型路由 | 1 |
| [crate-dependency-graph.md](./crate-dependency-graph.md) | **Crate 依赖关系图** — 21 个 Crate 的分层依赖矩阵与约束规则 | 2 |
| [data-model.md](./data-model.md) | **数据模型与存储架构** — SQLite 表结构、Qdrant 向量设计、文件系统布局 | 3 |
| [security-architecture.md](./security-architecture.md) | **安全架构 (v4.0)** — 12 层纵深防御、WASM 双计量沙箱、宿主边界凭证注入、SSRF 防护、密钥零化、泄漏检测 | 4 |
| [api-design.md](./api-design.md) | **API 设计** — RESTful 端点、FFI 接口、Cloud Relay API | 5 |
| [memory-architecture.md](./memory-architecture.md) | **记忆系统架构** — 三层记忆模型、提取/召回/衰减流水线、存储设计 | 6 |
| [decisions.md](./decisions.md) | **架构决策记录 (ADR)** — 20 项关键技术决策及理由 | 7 |

## 快速导航

- **想了解整体架构？** → [architecture-v3.0.md](./architecture-v3.0.md)
- **想了解模块依赖？** → [crate-dependency-graph.md](./crate-dependency-graph.md)
- **想了解数据库设计？** → [data-model.md](./data-model.md)
- **想了解安全设计？** → [security-architecture.md](./security-architecture.md)
- **想开发 API？** → [api-design.md](./api-design.md)
- **想了解记忆系统？** → [memory-architecture.md](./memory-architecture.md)
- **想了解为什么这样设计？** → [decisions.md](./decisions.md)

## v4.0 重点变更

### 安全架构 (6 层 → 12 层)

| 层 | 能力 | 参考来源 | 状态 |
|----|------|---------|------|
| L1 | Prompt 注入防御 (三层过滤) | 原有 | 增强 |
| L2 | WASM 双计量沙箱 (燃料+纪元) | OpenFang | **新增双计量** |
| L3 | 宿主边界凭证注入 | IronClaw | **新增** |
| L4 | 声明式权限能力模型 | 原有 | 增强为 TOML |
| L5 | DLP + Aho-Corasick 泄漏检测 | IronClaw | **增强** |
| L6 | SSRF 防护 + 网络白名单 | OpenFang | **新增 SSRF** |
| L7 | 路径穿越防护 | OpenFang | **新增** |
| L8 | 密钥零化 (Zeroizing) | OpenFang | **新增** |
| L9 | 循环守卫 + 子进程沙箱 | OpenFang | **新增** |
| L10 | Ed25519 签名验证 | OpenFang | **新增** |
| L11 | GCRA 速率限制 | OpenFang | **新增** |
| L12 | 哈希链审计日志 + 端点脱敏 | 原有 | 增强 |

### 其他架构变更

- **智能模型路由** (ADR-018): 按请求复杂度自动选择模型层级，参考 IronClaw
- **宿主边界凭证注入** (ADR-019): 密钥永不进入沙箱，参考 IronClaw
- **MCP 协议支持** (ADR-020): 支持 Model Context Protocol 工具集成标准
- **LLM Provider 扩展**: 新增 Google Gemini、中国市场 LLM 支持
- **预算追踪**: Per-Agent 预算上限与 Token 消耗告警
