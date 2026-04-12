import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus } from "lucide-react";
import { listChannels, listAgents } from "../lib/api";
import type { Channel, Agent } from "../lib/types";

const CHANNEL_STATUS_COLORS: Record<Channel["status"], string> = {
  connected: "#4ade80",
  disconnected: "#6b7280",
  error: "#f87171",
};

const CHANNEL_TYPE_LABELS: Record<Channel["channel_type"], string> = {
  telegram: "TG",
  lark: "Lark",
  slack: "Slack",
  whatsapp: "WA",
  discord: "DC",
  wecom: "WeCom",
};

export default function ChannelList({
  refreshKey = 0,
}: {
  refreshKey?: number;
}) {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("channel");

  const [channels, setChannels] = useState<Channel[]>([]);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadChannels = useCallback(async () => {
    try {
      setError(null);
      const [channelData, agentData] = await Promise.all([
        listChannels(),
        listAgents(),
      ]);
      setChannels(channelData);
      setAgents(agentData);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load channels",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadChannels();
  }, [loadChannels, refreshKey]);

  const agentMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agents) {
      map.set(a.id, a.name);
    }
    return map;
  }, [agents]);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    if (!q) return channels;
    return channels.filter(
      (c) =>
        c.name.toLowerCase().includes(q) ||
        c.channel_type.toLowerCase().includes(q),
    );
  }, [channels, search]);

  const handleSelect = useCallback(
    (id: string) => {
      setSearchParams({ channel: id });
    },
    [setSearchParams],
  );

  const handleAddChannel = useCallback(() => {
    setSearchParams({ add: "1" });
  }, [setSearchParams]);

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <div className="list-panel-header-row">
          <h2 className="list-panel-title">Connectors</h2>
          <button
            className="new-chat-btn"
            onClick={handleAddChannel}
            title="Add Channel"
            aria-label="Add Channel"
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
          aria-label="Search connectors"
          placeholder="Search connectors..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>
      <div className="list-panel-content">
        {loading && <p className="list-placeholder">Loading...</p>}
        {error && <p className="list-placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="list-placeholder">
            {search ? "No matches" : "No channels yet"}
          </p>
        )}
        {filtered.map((channel) => (
          <button
            key={channel.id}
            className={`channel-card ${selectedId === channel.id ? "selected" : ""}`}
            onClick={() => handleSelect(channel.id)}
            aria-label={`Select channel ${channel.name}`}
          >
            <span className="channel-type-badge">
              {CHANNEL_TYPE_LABELS[channel.channel_type]}
            </span>
            <div className="channel-card-info">
              <span className="channel-card-name">{channel.name}</span>
              <span className="channel-card-agent">
                {agentMap.get(channel.agent_id) ?? "Unbound"}
              </span>
            </div>
            <span
              className="channel-status-dot"
              style={{ background: CHANNEL_STATUS_COLORS[channel.status] }}
              title={channel.status}
            />
          </button>
        ))}
      </div>
    </aside>
  );
}
