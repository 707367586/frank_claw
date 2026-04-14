import Avatar from "./ui/Avatar";
import Badge from "./ui/Badge";
import Button from "./ui/Button";
import type { Agent } from "../lib/types";

const TONE_MAP = { working: "success", idle: "neutral", error: "error", offline: "warning" } as const;
const STATUS_LABEL = { working: "运行中", idle: "空闲", error: "错误", offline: "离线" } as const;

interface Props { agent: Agent; onEnter: () => void; onEdit: () => void }

export default function AgentGridCard({ agent, onEnter, onEdit }: Props) {
  return (
    <div className="agent-grid-card">
      <div className="agent-grid-card__head">
        <Avatar size={36} rounded="md" bg="var(--primary)">{agent.name.slice(0,2)}</Avatar>
        <div className="agent-grid-card__name-col">
          <div className="agent-grid-card__name">{agent.name}</div>
          <span className="agent-grid-card__model">{agent.model ?? "Claude Opus 4"}</span>
        </div>
        <Badge tone={TONE_MAP[agent.status]}>{STATUS_LABEL[agent.status]}</Badge>
      </div>
      <div className="agent-grid-card__meta">对话 28 · 产出 15 · 创建于 3月10日</div>
      <div className="agent-grid-card__actions">
        <Button variant="default" size="sm" onClick={onEnter}>进入</Button>
        <Button variant="outline" size="sm" onClick={onEdit}>编辑</Button>
      </div>
    </div>
  );
}
