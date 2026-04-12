import { useState, useEffect, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import { Pencil, Trash2, Pin, PinOff, Search } from "lucide-react";
import {
  getAgent,
  deleteAgent,
  listMemories,
  deleteMemory,
  pinMemory,
  getPermissionProfile,
} from "../lib/api";
import type { Agent, Memory, PermissionProfile } from "../lib/types";
import AgentForm from "../components/AgentForm";

type Tab = "profile" | "memories" | "permissions";

const STATUS_COLORS: Record<Agent["status"], string> = {
  idle: "#4ade80",
  working: "#facc15",
  error: "#f87171",
  offline: "#6b7280",
};

const MEMORY_TYPE_COLORS: Record<Memory["memory_type"], string> = {
  fact: "#60a5fa",
  preference: "#a78bfa",
  event: "#34d399",
  skill: "#fbbf24",
};

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("en-US", {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export default function ContactsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const agentId = searchParams.get("agent");

  const [agent, setAgent] = useState<Agent | null>(null);
  const [memories, setMemories] = useState<Memory[]>([]);
  const [permissions, setPermissions] = useState<PermissionProfile | null>(null);
  const [activeTab, setActiveTab] = useState<Tab>("profile");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [memorySearch, setMemorySearch] = useState("");

  const loadAgent = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const data = await getAgent(id);
      setAgent(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load agent");
    } finally {
      setLoading(false);
    }
  }, []);

  const loadMemories = useCallback(async (id: string) => {
    try {
      const data = await listMemories(id);
      setMemories(data);
    } catch {
      // Silently fail — memories tab will show empty
      setMemories([]);
    }
  }, []);

  const loadPermissions = useCallback(async (id: string) => {
    try {
      const data = await getPermissionProfile(id);
      setPermissions(data);
    } catch {
      setPermissions(null);
    }
  }, []);

  useEffect(() => {
    if (!agentId) {
      setAgent(null);
      setMemories([]);
      setPermissions(null);
      return;
    }
    loadAgent(agentId);
    loadMemories(agentId);
    loadPermissions(agentId);
  }, [agentId, loadAgent, loadMemories, loadPermissions]);

  const handleDelete = useCallback(async () => {
    if (!agent) return;
    const confirmed = window.confirm(
      `Are you sure you want to delete agent "${agent.name}"? This action cannot be undone.`,
    );
    if (!confirmed) return;

    try {
      await deleteAgent(agent.id);
      setSearchParams({});
      window.location.reload();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete agent");
    }
  }, [agent, setSearchParams]);

  const handleDeleteMemory = useCallback(
    async (memoryId: string) => {
      try {
        await deleteMemory(memoryId);
        setMemories((prev) => prev.filter((m) => m.id !== memoryId));
      } catch (err) {
        console.error("Failed to delete memory:", err);
      }
    },
    [],
  );

  const handlePinMemory = useCallback(
    async (memoryId: string) => {
      try {
        const updated = await pinMemory(memoryId);
        setMemories((prev) =>
          prev.map((m) => (m.id === memoryId ? updated : m)),
        );
      } catch (err) {
        console.error("Failed to pin memory:", err);
      }
    },
    [],
  );

  const filteredMemories = memorySearch
    ? memories.filter(
        (m) =>
          m.summary.toLowerCase().includes(memorySearch.toLowerCase()) ||
          m.memory_type.toLowerCase().includes(memorySearch.toLowerCase()),
      )
    : memories;

  // Empty state — no agent selected
  if (!agentId) {
    return (
      <div className="empty-state">
        <h2>Contacts</h2>
        <p>Select an agent from the sidebar to view details.</p>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="empty-state">
        <p>Loading agent...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="empty-state">
        <h2>Error</h2>
        <p>{error}</p>
      </div>
    );
  }

  if (!agent) {
    return (
      <div className="empty-state">
        <h2>Agent not found</h2>
        <p>The selected agent could not be loaded.</p>
      </div>
    );
  }

  return (
    <div className="agent-detail">
      {/* Header */}
      <div className="agent-detail-header">
        <div className="agent-detail-header-left">
          <div className="agent-detail-avatar">
            {agent.name.charAt(0).toUpperCase()}
          </div>
          <div>
            <h2 className="agent-detail-name">{agent.name}</h2>
            <span className="agent-detail-role">{agent.role}</span>
          </div>
          <span
            className="agent-status-badge"
            style={{
              background: STATUS_COLORS[agent.status],
              color: agent.status === "working" ? "#1a1a2e" : "#fff",
            }}
          >
            {agent.status}
          </span>
        </div>
        <div className="agent-detail-actions">
          <button
            className="btn-icon"
            onClick={() => setEditing(true)}
            title="Edit agent"
            aria-label="Edit agent"
          >
            <Pencil size={16} />
          </button>
          <button
            className="btn-icon btn-danger"
            onClick={handleDelete}
            title="Delete agent"
            aria-label="Delete agent"
          >
            <Trash2 size={16} />
          </button>
        </div>
      </div>

      {/* Tabs */}
      <div className="tabs">
        {(["profile", "memories", "permissions"] as Tab[]).map((tab) => (
          <button
            key={tab}
            className={`tab ${activeTab === tab ? "active" : ""}`}
            onClick={() => setActiveTab(tab)}
            aria-label={`${tab} tab`}
          >
            {tab.charAt(0).toUpperCase() + tab.slice(1)}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="tab-content">
        {activeTab === "profile" && (
          <div className="profile-section">
            <div className="profile-field">
              <span className="profile-field-label">Name</span>
              <span className="profile-field-value">{agent.name}</span>
            </div>
            <div className="profile-field">
              <span className="profile-field-label">Role</span>
              <span className="profile-field-value">{agent.role}</span>
            </div>
            <div className="profile-field">
              <span className="profile-field-label">Model</span>
              <span className="profile-field-value">
                {agent.model_id ?? "Default"}
              </span>
            </div>
            <div className="profile-field">
              <span className="profile-field-label">System Prompt</span>
              <span className="profile-field-value profile-field-pre">
                {agent.system_prompt || "(none)"}
              </span>
            </div>
            <div className="profile-field">
              <span className="profile-field-label">Created</span>
              <span className="profile-field-value">
                {formatDate(agent.created_at)}
              </span>
            </div>
            <div className="profile-field">
              <span className="profile-field-label">Updated</span>
              <span className="profile-field-value">
                {formatDate(agent.updated_at)}
              </span>
            </div>
          </div>
        )}

        {activeTab === "memories" && (
          <div className="memories-section">
            <div className="memories-search">
              <Search size={14} className="search-icon" />
              <input
                type="text"
                className="search-input"
                placeholder="Search memories..."
                aria-label="Search memories"
                value={memorySearch}
                onChange={(e) => setMemorySearch(e.target.value)}
              />
            </div>
            {filteredMemories.length === 0 ? (
              <p className="list-placeholder">
                {memorySearch ? "No matching memories" : "No memories yet"}
              </p>
            ) : (
              <div className="memory-list">
                {filteredMemories.map((mem) => (
                  <div key={mem.id} className="memory-item">
                    <div className="memory-item-top">
                      <span
                        className="memory-type-badge"
                        style={{ background: MEMORY_TYPE_COLORS[mem.memory_type] }}
                      >
                        {mem.memory_type}
                      </span>
                      <div className="memory-item-actions">
                        <button
                          className="btn-icon-sm"
                          onClick={() => handlePinMemory(mem.id)}
                          title={mem.pinned ? "Unpin" : "Pin"}
                          aria-label={mem.pinned ? "Unpin memory" : "Pin memory"}
                        >
                          {mem.pinned ? <PinOff size={14} /> : <Pin size={14} />}
                        </button>
                        <button
                          className="btn-icon-sm btn-danger"
                          onClick={() => handleDeleteMemory(mem.id)}
                          title="Delete memory"
                          aria-label="Delete memory"
                        >
                          <Trash2 size={14} />
                        </button>
                      </div>
                    </div>
                    <p className="memory-summary">{mem.summary}</p>
                    <div className="memory-meta">
                      <span>Importance: {mem.importance.toFixed(1)}</span>
                      <span>Freshness: {mem.freshness.toFixed(1)}</span>
                      <span>{formatDate(mem.created_at)}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "permissions" && (
          <div className="permissions-section">
            {permissions ? (
              <>
                <div className="profile-field">
                  <span className="profile-field-label">Trust Level</span>
                  <span className="trust-badge">{permissions.trust_level}</span>
                </div>
                <div className="profile-field">
                  <span className="profile-field-label">Safety Incidents</span>
                  <span className="profile-field-value">
                    {permissions.safety_incidents}
                  </span>
                </div>
                <h4 className="permissions-heading">Capability Scores</h4>
                {Object.entries(permissions.capability_scores).length === 0 ? (
                  <p className="list-placeholder">No capabilities configured</p>
                ) : (
                  <div className="capability-list">
                    {Object.entries(permissions.capability_scores).map(
                      ([key, value]) => (
                        <div key={key} className="capability-item">
                          <span className="capability-name">{key}</span>
                          <span className="capability-value">
                            {String(value)}
                          </span>
                        </div>
                      ),
                    )}
                  </div>
                )}
              </>
            ) : (
              <p className="list-placeholder">
                No permission profile available
              </p>
            )}
          </div>
        )}
      </div>

      {/* Edit modal */}
      {editing && (
        <AgentForm
          agent={agent}
          onSaved={() => {
            setEditing(false);
            if (agentId) loadAgent(agentId);
          }}
          onCancel={() => setEditing(false)}
        />
      )}
    </div>
  );
}
