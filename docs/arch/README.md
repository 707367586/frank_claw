# ClawX 架构文档索引

**架构版本:** v6.0 | **日期:** 2026-04-21

当前版本 v6.0：`hermes_bridge`（Python FastAPI）替代 vendored picoclaw。详见 [ADR-038](./decisions.md#adr-038-2026-04-21--后端从-vendored-picoclaw-切换到-hermes-agent)。

---

| 文档 | 内容 |
|------|------|
| [architecture.md](./architecture.md) | **系统总体架构 v6.0** — hermes-agent 后端、`hermes_bridge` 适配层、前端 / 后端 / 协议分层 |
| [api-design.md](./api-design.md) | **API 设计 v6.0** — REST `/api/hermes/info` + `/api/sessions/skills/tools`，WS `/hermes/ws` 子协议鉴权与帧格式 |
| [decisions.md](./decisions.md) | **架构决策记录 (ADR)** — 从 ADR-001 至 ADR-038 全部决策 |

## 快速导航

- **想了解当前整体架构？** → [architecture.md](./architecture.md)
- **想开发 / 消费 API？** → [api-design.md](./api-design.md)
- **想了解为什么这样设计？** → [decisions.md](./decisions.md) (最新 = ADR-038)
