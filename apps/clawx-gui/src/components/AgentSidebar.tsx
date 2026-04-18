import { useState, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus, Menu, ChevronDown } from "lucide-react";
import Input from "./ui/Input";
import IconButton from "./ui/IconButton";
import Avatar from "./ui/Avatar";
import AgentTemplateModal from "./AgentTemplateModal";
import { useAgents } from "../lib/store";
import type { Agent } from "../lib/types";

const STATUS_DESC: Record<Agent["status"], string> = {
  working: "Running",
  idle:    "Idle",
  error:   "Error",
  offline: "Offline",
};

const EMOJI: Record<string, string> = {
  dev: "💻", research: "🔍", writing: "✍️", data: "📊",
};

function pickEmoji(agent: Agent): string {
  const key = (agent.role ?? "").toLowerCase();
  if (key.includes("code") || key.includes("dev")) return EMOJI.dev;
  if (key.includes("research")) return EMOJI.research;
  if (key.includes("writ")) return EMOJI.writing;
  if (key.includes("data")) return EMOJI.data;
  return agent.name.slice(0, 1);
}

export default function AgentSidebar() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("agent");
  const { agents, loading, error } = useAgents();
  const [search, setSearch] = useState("");
  const [openNew, setOpenNew] = useState(false);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    return !q ? agents : agents.filter((a) => a.name.toLowerCase().includes(q) || (a.role ?? "").toLowerCase().includes(q));
  }, [agents, search]);

  const handleSelect = useCallback((id: string) => {
    const params = new URLSearchParams(searchParams);
    params.set("agent", id);
    setSearchParams(params);
  }, [searchParams, setSearchParams]);

  return (
    <aside className="agent-sidebar">
      <header className="agent-sidebar__head">
        <div className="agent-sidebar__brand">
          <span className="agent-sidebar__brand-name">ZettClaw</span>
          <ChevronDown size={14} />
        </div>
        <IconButton icon={<Menu size={16} />} aria-label="菜单" variant="ghost" size="sm" />
      </header>

      <div className="agent-sidebar__search">
        <Input
          size="sm"
          leftIcon={<Search size={14} />}
          placeholder="搜索 Agent..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
        <IconButton icon={<Plus size={14} />} aria-label="新建 Agent" variant="default" size="sm" onClick={() => setOpenNew(true)} />
      </div>

      <div className="agent-sidebar__list">
        {loading && <p className="agent-sidebar__placeholder">加载中...</p>}
        {error && <p className="agent-sidebar__placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="agent-sidebar__placeholder">{search ? "无匹配" : "暂无 Agent"}</p>
        )}
        {filtered.map((agent) => (
          <button
            key={agent.id}
            className={`agent-item ${selectedId === agent.id ? "is-active" : ""}`}
            onClick={() => handleSelect(agent.id)}
          >
            <Avatar size={40} rounded="md" bg="var(--primary)">{pickEmoji(agent)}</Avatar>
            <div className="agent-item__text">
              <span className="agent-item__name">{agent.name}</span>
              <span className="agent-item__status">{STATUS_DESC[agent.status]}</span>
            </div>
          </button>
        ))}
      </div>
      <AgentTemplateModal open={openNew} onClose={() => setOpenNew(false)} />
    </aside>
  );
}
