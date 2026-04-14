import { useState, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus, Menu, ChevronDown } from "lucide-react";
import { STATUS_COLORS } from "../lib/constants";
import { useAgents } from "../lib/store";
import type { Agent } from "../lib/types";

function getStatusDetail(agent: Agent): string {
  switch (agent.status) {
    case "working": return "Running · 3 running";
    case "idle": return "Idle";
    case "error": return "Error";
    case "offline": return "Offline";
    default: return agent.status;
  }
}

export default function AgentSidebar() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("agent");

  const { agents, loading, error } = useAgents();
  const [search, setSearch] = useState("");

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    if (!q) return agents;
    return agents.filter(
      (a) =>
        a.name.toLowerCase().includes(q) || a.role.toLowerCase().includes(q),
    );
  }, [agents, search]);

  const handleSelect = useCallback(
    (id: string) => {
      const params = new URLSearchParams(searchParams);
      params.set("agent", id);
      setSearchParams(params);
    },
    [searchParams, setSearchParams],
  );

  return (
    <aside className="agent-sidebar">
      {/* Header */}
      <div className="sidebar-header">
        <Menu size={20} className="sidebar-menu-icon" />
        <div className="sidebar-brand">
          <span className="sidebar-brand-name">ZettClaw</span>
          <ChevronDown size={14} className="sidebar-brand-chevron" />
        </div>
      </div>

      {/* Search + Add */}
      <div className="sidebar-actions">
        <div className="sidebar-search">
          <Search size={14} className="sidebar-search-icon" />
          <input
            type="text"
            className="sidebar-search-input"
            aria-label="Search agents"
            placeholder="Search agents..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
          />
        </div>
        <button className="sidebar-add-btn" aria-label="Add agent">
          <Plus size={16} />
        </button>
      </div>

      {/* Agent list */}
      <div className="sidebar-agent-list">
        {loading && <p className="sidebar-placeholder">Loading...</p>}
        {error && <p className="sidebar-placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="sidebar-placeholder">
            {search ? "No matches" : "No agents yet"}
          </p>
        )}
        {filtered.map((agent) => (
          <button
            key={agent.id}
            className={`sidebar-agent-item ${selectedId === agent.id ? "selected" : ""}`}
            onClick={() => handleSelect(agent.id)}
            aria-label={`Select agent ${agent.name}`}
          >
            <span
              className="sidebar-agent-dot"
              style={{ background: STATUS_COLORS[agent.status] }}
            />
            <div className="sidebar-agent-info">
              <span className="sidebar-agent-name">{agent.name}</span>
              <span className="sidebar-agent-status">{getStatusDetail(agent)}</span>
            </div>
          </button>
        ))}
      </div>

      {/* Bottom divider */}
      <div className="sidebar-divider" />
    </aside>
  );
}
