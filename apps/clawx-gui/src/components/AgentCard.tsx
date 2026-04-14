import type { Agent } from "../lib/types";

interface AgentCardProps {
  agent: Agent;
  onEnter: (agent: Agent) => void;
  onEdit: (agent: Agent) => void;
}

const STATUS_LABELS: Record<Agent["status"], string> = {
  idle: "空闲",
  working: "运行中",
  error: "异常",
  offline: "离线",
};

const STATUS_BG: Record<Agent["status"], string> = {
  idle: "var(--secondary)",
  working: "#22C55E22",
  error: "#EF444422",
  offline: "var(--secondary)",
};

const STATUS_FG: Record<Agent["status"], string> = {
  idle: "var(--muted-foreground)",
  working: "var(--success)",
  error: "var(--error)",
  offline: "var(--muted-foreground)",
};

export default function AgentCard({ agent, onEnter, onEdit }: AgentCardProps) {
  return (
    <div className="agent-card-item">
      <div className="agent-card-header">
        <span className="agent-card-name">{agent.name}</span>
        <span
          className="agent-card-status-badge"
          style={{ background: STATUS_BG[agent.status], color: STATUS_FG[agent.status] }}
        >
          {STATUS_LABELS[agent.status]}
        </span>
      </div>
      <div className="agent-card-body">
        <div className="agent-card-model">{agent.model_id || "Claude Opus 4"}</div>
        <div className="agent-card-stats">
          对话 {Math.floor(Math.random() * 20)} · 产出 {Math.floor(Math.random() * 10)}
        </div>
      </div>
      <div className="agent-card-actions">
        <button className="btn-primary-pill agent-card-btn" onClick={() => onEnter(agent)}>进入</button>
        <button className="btn-outline-pill agent-card-btn" onClick={() => onEdit(agent)}>编辑</button>
      </div>
    </div>
  );
}
