import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus } from "lucide-react";
import { listConversations, createConversation } from "../lib/api";
import { useAgents } from "../lib/store";
import type { Conversation } from "../lib/types";

function timeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const mins = Math.floor(diff / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

export default function ConversationList() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("conv");

  const { agents } = useAgents();
  const [conversations, setConversations] = useState<Conversation[]>([]);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const agentMap = useMemo(() => {
    const m = new Map<string, typeof agents[number]>();
    for (const a of agents) m.set(a.id, a);
    return m;
  }, [agents]);

  const loadData = useCallback(async () => {
    try {
      setError(null);
      // Backend requires agent_id, so load conversations for all agents
      const allConvs: Conversation[] = [];
      await Promise.all(
        agents.map(async (a) => {
          try {
            const convs = await listConversations(a.id);
            allConvs.push(...convs);
          } catch {
            // skip failed agents
          }
        }),
      );
      setConversations(allConvs);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load");
    } finally {
      setLoading(false);
    }
  }, [agents]);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    const list = q
      ? conversations.filter((c) => {
          const agent = agentMap.get(c.agent_id);
          return (
            c.title.toLowerCase().includes(q) ||
            agent?.name.toLowerCase().includes(q)
          );
        })
      : conversations;
    return [...list].sort(
      (a, b) =>
        new Date(b.updated_at).getTime() - new Date(a.updated_at).getTime(),
    );
  }, [conversations, search, agentMap]);

  const handleSelect = useCallback(
    (id: string) => {
      setSearchParams({ conv: id });
    },
    [setSearchParams],
  );

  const handleNewChat = useCallback(async () => {
    if (agents.length === 0) return;
    try {
      // Use the first agent by default; a future enhancement could show a picker
      const conv = await createConversation(agents[0].id);
      setConversations((prev) => [conv, ...prev]);
      setSearchParams({ conv: conv.id });
    } catch (err) {
      console.error("Failed to create conversation:", err);
    }
  }, [agents, setSearchParams]);

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <div className="list-panel-header-row">
          <h2 className="list-panel-title">Conversations</h2>
          <button
            className="new-chat-btn"
            onClick={handleNewChat}
            title="New Chat"
            disabled={agents.length === 0}
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
          aria-label="Search conversations"
          placeholder="Search conversations..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>
      <div className="list-panel-content">
        {loading && <p className="list-placeholder">Loading...</p>}
        {error && <p className="list-placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="list-placeholder">
            {search ? "No matches" : "No conversations yet"}
          </p>
        )}
        {filtered.map((conv) => {
          const agent = agentMap.get(conv.agent_id);
          return (
            <button
              key={conv.id}
              className={`conv-item ${selectedId === conv.id ? "selected" : ""}`}
              onClick={() => handleSelect(conv.id)}
            >
              <div className="conv-item-top">
                <span className="conv-agent-name">
                  {agent?.name ?? "Agent"}
                </span>
                <span className="conv-time">{timeAgo(conv.updated_at)}</span>
              </div>
              <div className="conv-title">
                {conv.title || "New conversation"}
              </div>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
