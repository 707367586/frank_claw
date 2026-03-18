# ClawX 架构文档索引

**对应 PRD:** v2.0 | **架构版本:** v4.1 | **日期:** 2026-03-18

---

## 文档目录

| 文档 | 内容 | 阅读顺序 |
|------|------|---------|
| [architecture.md](./architecture.md) | **系统总体架构 v4.0** — 分层架构、模块职责、核心数据流、部署架构、技术栈选型 | 1 |
| [crate-dependency-graph.md](./crate-dependency-graph.md) | **Crate 依赖关系图** — Crate 分层依赖矩阵与约束规则 | 2 |
| [data-model.md](./data-model.md) | **数据模型与存储架构** — SQLite 表结构、Qdrant 向量设计、文件系统布局 | 3 |
| [security-architecture.md](./security-architecture.md) | **安全架构 v4.0** — 威胁模型、三级沙箱、12 层纵深防御、DLP、审计日志 | 4 |
| [memory-architecture.md](./memory-architecture.md) | **记忆系统架构** — 三层记忆模型、艾宾浩斯衰减、语义召回、Trait 接口 | 5 |
| [autonomy-architecture.md](./autonomy-architecture.md) | **自主性架构** — ReAct 推理循环、多 Agent 协作、自我反思、信任渐进 | 6 |
| [api-design.md](./api-design.md) | **API 设计 v4.1** — 本地控制平面、RESTful 端点、FFI 边界 | 7 |
| [decisions.md](./decisions.md) | **架构决策记录 (ADR)** — 关键技术决策及理由 | 8 |

## 快速导航

- **想了解整体架构？** → [architecture.md](./architecture.md)
- **想了解模块依赖？** → [crate-dependency-graph.md](./crate-dependency-graph.md)
- **想了解数据库设计？** → [data-model.md](./data-model.md)
- **想了解安全设计？** → [security-architecture.md](./security-architecture.md)
- **想了解记忆系统？** → [memory-architecture.md](./memory-architecture.md)
- **想了解 Agent 自主性？** → [autonomy-architecture.md](./autonomy-architecture.md)
- **想开发 API？** → [api-design.md](./api-design.md)
- **想了解为什么这样设计？** → [decisions.md](./decisions.md)
