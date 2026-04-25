import { useState, type ComponentType } from "react";
import {
  Menu, ChevronDown, Plus, Search, Trash2,
  Code2, PenTool, BarChart3, Bot, MessageSquare, FileText, Lightbulb,
  Sparkles, Database, Globe, Wrench,
} from "lucide-react";
import IconButton from "./ui/IconButton";
import Input from "./ui/Input";
import CreateAgentModal from "./CreateAgentModal";
import { useClaw } from "../lib/store";

const ICONS: Record<string, ComponentType<{ size?: number }>> = {
  Code2, Search, PenTool, BarChart3, Bot, MessageSquare,
  FileText, Lightbulb, Sparkles, Database, Globe, Wrench,
};

export default function AgentSidebar() {
  const claw = useClaw();
  const [query, setQuery] = useState("");
  const [modalOpen, setModalOpen] = useState(false);

  const filtered = claw.agents.filter((a) =>
    a.name.toLowerCase().includes(query.trim().toLowerCase()),
  );

  return (
    <aside className="agent-sidebar" aria-label="Agent list">
      <div className="agent-sidebar__head">
        <button type="button" className="agent-sidebar__brand" aria-label="切换工作区">
          <Menu size={16} />
          <span>ZettClaw</span>
          <ChevronDown size={14} />
        </button>
      </div>

      <div className="agent-sidebar__search">
        <Input
          size="sm"
          leftIcon={<Search size={14} />}
          placeholder="搜索 Agent..."
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <IconButton
          icon={<Plus size={16} />}
          aria-label="新建 Agent"
          variant="ghost"
          size="sm"
          onClick={() => setModalOpen(true)}
        />
      </div>

      <div className="agent-sidebar__list">
        {filtered.length === 0 ? (
          <div className="agent-sidebar__placeholder">未找到匹配的 Agent</div>
        ) : (
          filtered.map((a) => {
            const Icon = ICONS[a.icon] ?? Bot;
            const isActive = a.id === claw.activeAgentId;
            const status = isActive && claw.chat.typing ? "running" : "idle";
            const statusText = status === "running" ? "Running" : "Idle";
            return (
              <div key={a.id} className="agent-item-wrap">
                <button
                  type="button"
                  aria-label={a.name}
                  className={`agent-item ${isActive ? "is-active" : ""}`.trim()}
                  onClick={() => claw.selectAgent(a.id)}
                >
                  <span
                    className="agent-item__avatar"
                    style={{ background: a.color }}
                    aria-hidden
                  >
                    <Icon size={16} />
                  </span>
                  <span className="agent-item__text">
                    <span className="agent-item__name">{a.name}</span>
                    <span className="agent-item__status">
                      <span className={`agent-item__dot agent-item__dot--${status}`} />
                      {statusText}
                    </span>
                  </span>
                </button>
                <span
                  className="agent-item__delete"
                  role="menuitem"
                  tabIndex={0}
                  aria-label={`删除 ${a.name}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    if (window.confirm(`删除 Agent 「${a.name}」？此操作不可撤销。`)) {
                      void claw.deleteAgent(a.id);
                    }
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      e.stopPropagation();
                      if (window.confirm(`删除 Agent 「${a.name}」？此操作不可撤销。`)) {
                        void claw.deleteAgent(a.id);
                      }
                    }
                  }}
                >
                  <Trash2 size={14} />
                </span>
              </div>
            );
          })
        )}
      </div>

      <CreateAgentModal open={modalOpen} onClose={() => setModalOpen(false)} />
    </aside>
  );
}
