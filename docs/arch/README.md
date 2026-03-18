# ClawX 架构文档索引

**对应 PRD:** v2.0 | **架构版本:** v3.0 | **日期:** 2026-03-17

---

## 文档目录

| 文档 | 内容 | 阅读顺序 |
|------|------|---------|
| [architecture-v3.0.md](./architecture-v3.0.md) | **系统总体架构** — 分层架构、模块职责、核心数据流、部署架构、技术栈选型 | 1 |
| [crate-dependency-graph.md](./crate-dependency-graph.md) | **Crate 依赖关系图** — 21 个 Crate 的分层依赖矩阵与约束规则 | 2 |
| [data-model.md](./data-model.md) | **数据模型与存储架构** — SQLite 表结构、Qdrant 向量设计、文件系统布局 | 3 |
| [security-architecture.md](./security-architecture.md) | **安全架构** — 威胁模型、三级沙箱、DLP、Prompt 注入防御、审计日志 | 4 |
| [api-design.md](./api-design.md) | **API 设计** — RESTful 端点、FFI 接口、请求/响应格式 | 5 |
| [decisions.md](./decisions.md) | **架构决策记录 (ADR)** — 11 项关键技术决策及理由 | 6 |

## 快速导航

- **想了解整体架构？** → [architecture-v3.0.md](./architecture-v3.0.md)
- **想了解模块依赖？** → [crate-dependency-graph.md](./crate-dependency-graph.md)
- **想了解数据库设计？** → [data-model.md](./data-model.md)
- **想了解安全设计？** → [security-architecture.md](./security-architecture.md)
- **想开发 API？** → [api-design.md](./api-design.md)
- **想了解为什么这样设计？** → [decisions.md](./decisions.md)
