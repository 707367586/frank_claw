import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus } from "lucide-react";
import { listAgents } from "../lib/api";
import { STATUS_COLORS } from "../lib/constants";
import type { Agent } from "../lib/types";

export default function AgentList({
  onCreateAgent,
  refreshKey = 0,
}: {
  onCreateAgent: () => void;
  refreshKey?: number;
}) {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("agent");

  const [agents, setAgents] = useState<Agent[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadAgents = useCallback(async () => {
    try {
      setError(null);
      const data = await listAgents();
      setAgents(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load agents");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadAgents();
  }, [loadAgents, refreshKey]);

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
      setSearchParams({ agent: id });
    },
    [setSearchParams],
  );

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <div className="list-panel-header-row">
          <h2 className="list-panel-title">Contacts</h2>
          <button
            className="new-chat-btn"
            onClick={onCreateAgent}
            title="Create Agent"
            aria-label="Create Agent"
          >
            <Plus size={16} />
          </button>
        </div>
      </div>
      <div className="list-panel-search">
        <Search size={14} className="search-icon" />
        <input
          type="text"
          className="search-input"
          aria-label="Search contacts"
          placeholder="Search contacts..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>
      <div className="list-panel-content">
        {loading && <p className="list-placeholder">Loading...</p>}
        {error && <p className="list-placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="list-placeholder">
            {search ? "No matches" : "No agents yet"}
          </p>
        )}
        {filtered.map((agent) => (
          <button
            key={agent.id}
            className={`agent-card ${selectedId === agent.id ? "selected" : ""}`}
            onClick={() => handleSelect(agent.id)}
            aria-label={`Select agent ${agent.name}`}
          >
            <div className="agent-card-avatar">
              {agent.name.charAt(0).toUpperCase()}
            </div>
            <div className="agent-card-info">
              <span className="agent-card-name">{agent.name}</span>
              <span className="agent-card-role">{agent.role}</span>
            </div>
            <span
              className="agent-status-dot"
              style={{ background: STATUS_COLORS[agent.status] }}
              title={agent.status}
            />
          </button>
        ))}
      </div>
    </aside>
  );
}
