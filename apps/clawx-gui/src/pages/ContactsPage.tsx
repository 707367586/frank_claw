import { useState, useEffect, useCallback } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";
import {
  Trash2,
  Pin,
  PinOff,
  Search,
  FolderOpen,
  Brain,
  MessageSquare,
  Pencil,
  Zap,
} from "lucide-react";
import {
  getAgent,
  deleteAgent,
  listMemories,
  deleteMemory,
  pinMemory,
  listKnowledgeSources,
  listSkills,
  createConversation,
} from "../lib/api";
import { STATUS_COLORS, MEMORY_TYPE_COLORS } from "../lib/constants";
import type { Agent, Memory, KnowledgeSource, Skill } from "../lib/types";
import AgentForm from "../components/AgentForm";

// Avatar background colors derived from agent name
const AVATAR_COLORS = [
  "#7c5cfc",
  "#3b82f6",
  "#10b981",
  "#f59e0b",
  "#ef4444",
  "#8b5cf6",
  "#06b6d4",
  "#ec4899",
];

function getAvatarColor(name: string): string {
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = name.charCodeAt(i) + ((hash << 5) - hash);
  }
  return AVATAR_COLORS[Math.abs(hash) % AVATAR_COLORS.length];
}

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "long",
    day: "numeric",
  });
}

// Extract filename from path
function getFileName(path: string): string {
  const parts = path.split("/");
  return parts[parts.length - 1] || path;
}

// Extract parent directory as description
function getFileDescription(path: string): string {
  const parts = path.split("/");
  if (parts.length > 1) {
    return parts.slice(0, -1).join("/");
  }
  return "";
}

export default function ContactsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const navigate = useNavigate();
  const agentId = searchParams.get("agent");

  const [agent, setAgent] = useState<Agent | null>(null);
  const [memories, setMemories] = useState<Memory[]>([]);
  const [knowledgeSources, setKnowledgeSources] = useState<KnowledgeSource[]>(
    [],
  );
  const [skills, setSkills] = useState<Skill[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [editing, setEditing] = useState(false);
  const [memorySearch, setMemorySearch] = useState("");

  // Mock stats (no dedicated endpoint yet)
  const mockStats = { conversations: 28, artifacts: 15 };

  // Mock capability tags
  const capabilityTags = ["Python", "Bash", "TypeScript", "SQL", "Docker"];

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
      setMemories([]);
    }
  }, []);

  const loadKnowledgeSources = useCallback(async (agentId: string) => {
    try {
      const all = await listKnowledgeSources();
      setKnowledgeSources(all.filter((ks) => ks.agent_id === agentId));
    } catch {
      setKnowledgeSources([]);
    }
  }, []);

  const loadSkills = useCallback(async () => {
    try {
      const data = await listSkills();
      setSkills(data.filter((s) => s.status === "enabled"));
    } catch {
      setSkills([]);
    }
  }, []);

  useEffect(() => {
    if (!agentId) {
      setAgent(null);
      setMemories([]);
      setKnowledgeSources([]);
      setSkills([]);
      return;
    }
    loadAgent(agentId);
    loadMemories(agentId);
    loadKnowledgeSources(agentId);
    loadSkills();
  }, [agentId, loadAgent, loadMemories, loadKnowledgeSources, loadSkills]);

  const handleDelete = useCallback(async () => {
    if (!agent) return;
    const confirmed = window.confirm(
      `Are you sure you want to delete agent "${agent.name}"? This action cannot be undone.`,
    );
    if (!confirmed) return;

    try {
      await deleteAgent(agent.id);
      setAgent(null);
      setMemories([]);
      setKnowledgeSources([]);
      setSearchParams({});
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete agent");
    }
  }, [agent, setSearchParams]);

  const handleDeleteMemory = useCallback(async (memoryId: string) => {
    setMutationError(null);
    try {
      await deleteMemory(memoryId);
      setMemories((prev) => prev.filter((m) => m.id !== memoryId));
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to delete memory",
      );
    }
  }, []);

  const handlePinMemory = useCallback(async (memoryId: string) => {
    setMutationError(null);
    try {
      const updated = await pinMemory(memoryId);
      setMemories((prev) =>
        prev.map((m) => (m.id === memoryId ? updated : m)),
      );
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to update memory pin",
      );
    }
  }, []);

  const handleSendMessage = useCallback(async () => {
    if (!agent) return;
    try {
      const conv = await createConversation(agent.id);
      navigate(`/chat?agent=${agent.id}&conv=${conv.id}`);
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to create conversation",
      );
    }
  }, [agent, navigate]);

  const filteredMemories = memorySearch
    ? memories.filter(
        (m) =>
          m.summary.toLowerCase().includes(memorySearch.toLowerCase()) ||
          m.memory_type.toLowerCase().includes(memorySearch.toLowerCase()),
      )
    : memories;

  // Empty state -- no agent selected
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

  if (error && !agent) {
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
    <div className="contacts-detail">
      {/* Agent Header */}
      <div className="contacts-header">
        <div className="contacts-header-left">
          <div
            className="contacts-avatar"
            style={{ background: getAvatarColor(agent.name) }}
          >
            {agent.name.charAt(0).toUpperCase()}
          </div>
          <div className="contacts-info">
            <div className="contacts-name-row">
              <h2 className="contacts-name">{agent.name}</h2>
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
            <span className="contacts-model">
              {agent.model_id ?? "Default Model"}
            </span>
            <span className="contacts-time">
              创建于 {formatDate(agent.created_at)}
            </span>
          </div>
        </div>
        <div className="contacts-actions">
          <button className="btn-primary" onClick={handleSendMessage}>
            <MessageSquare size={14} />
            发消息
          </button>
          <button className="btn-outline" onClick={() => setEditing(true)}>
            <Pencil size={14} />
            编辑
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

      {/* Description */}
      {agent.system_prompt && (
        <p className="contacts-description">{agent.role}</p>
      )}

      {/* Error banner */}
      {(error || mutationError) && (
        <p className="form-error" style={{ margin: "0 24px" }}>
          {error || mutationError}
        </p>
      )}

      {/* Scrollable content area */}
      <div className="contacts-body">
        {/* Stats Overview */}
        <section className="contacts-section">
          <h3 className="contacts-section-title">统计概览</h3>
          <div className="contacts-stats">
            <div className="stat-card">
              <div className="stat-number">{mockStats.conversations}</div>
              <div className="stat-label">对话</div>
            </div>
            <div className="stat-card">
              <div className="stat-number">{mockStats.artifacts}</div>
              <div className="stat-label">产物</div>
            </div>
          </div>
          <div className="contacts-tags">
            {capabilityTags.map((tag) => (
              <span key={tag} className="skill-tag">
                {tag}
              </span>
            ))}
          </div>
        </section>

        {/* Knowledge Documents */}
        <section className="contacts-section">
          <h3 className="contacts-section-title">
            <FolderOpen size={16} />
            知识文档
          </h3>
          {knowledgeSources.length === 0 ? (
            <p className="list-placeholder">暂无知识文档</p>
          ) : (
            <div className="contacts-knowledge-list">
              {knowledgeSources.map((ks) => (
                <div key={ks.id} className="contacts-knowledge-item">
                  <div className="contacts-knowledge-icon">
                    <FolderOpen size={16} />
                  </div>
                  <div className="contacts-knowledge-info">
                    <span className="contacts-knowledge-name">
                      {getFileName(ks.path)}
                    </span>
                    <span className="contacts-knowledge-desc">
                      {getFileDescription(ks.path)}
                      {ks.doc_count > 0 && ` · ${ks.doc_count} docs`}
                      {ks.chunk_count > 0 && ` · ${ks.chunk_count} chunks`}
                    </span>
                  </div>
                  <span
                    className="contacts-knowledge-status"
                    data-status={ks.status}
                  >
                    {ks.status}
                  </span>
                </div>
              ))}
            </div>
          )}
        </section>

        {/* Memories */}
        <section className="contacts-section">
          <h3 className="contacts-section-title">
            <Brain size={16} />
            记忆
            <span className="contacts-section-count">{memories.length}</span>
          </h3>
          <div className="memories-search" style={{ marginBottom: 12 }}>
            <Search size={14} className="search-icon" />
            <input
              type="text"
              className="search-input"
              placeholder="搜索记忆..."
              aria-label="Search memories"
              value={memorySearch}
              onChange={(e) => setMemorySearch(e.target.value)}
            />
          </div>
          {filteredMemories.length === 0 ? (
            <p className="list-placeholder">
              {memorySearch ? "未找到匹配的记忆" : "暂无记忆"}
            </p>
          ) : (
            <div className="contacts-memory-list">
              {filteredMemories.map((mem) => (
                <div key={mem.id} className="contacts-memory-item">
                  <div className="contacts-memory-icon">
                    <Brain size={14} />
                  </div>
                  <div className="contacts-memory-content">
                    <div className="contacts-memory-top">
                      <span
                        className="memory-type-badge"
                        style={{
                          background: MEMORY_TYPE_COLORS[mem.memory_type],
                        }}
                      >
                        {mem.memory_type}
                      </span>
                      {mem.pinned && (
                        <Pin size={12} className="contacts-memory-pinned" />
                      )}
                    </div>
                    <p className="contacts-memory-text">{mem.summary}</p>
                  </div>
                  <div className="contacts-memory-actions">
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
              ))}
            </div>
          )}
        </section>

        {/* Skills */}
        <section className="contacts-section">
          <h3 className="contacts-section-title">
            <Zap size={16} />
            Skills
          </h3>
          {skills.length === 0 ? (
            <p className="list-placeholder">暂无已启用的技能</p>
          ) : (
            <div className="contacts-skills-list">
              {skills.map((skill) => (
                <div key={skill.id} className="contacts-skill-item">
                  <div className="contacts-skill-icon">
                    <Zap size={14} />
                  </div>
                  <div className="contacts-skill-info">
                    <span className="contacts-skill-name">{skill.name}</span>
                    <span className="contacts-skill-desc">
                      {skill.description}
                    </span>
                  </div>
                  <span className="contacts-skill-version">
                    v{skill.version}
                  </span>
                </div>
              ))}
            </div>
          )}
        </section>
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
