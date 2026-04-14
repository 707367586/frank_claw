import { useState } from "react";
import { useSearchParams } from "react-router-dom";
import { LayoutGrid, Plus, Search } from "lucide-react";
import { useAgents } from "../lib/store";
import type { Agent } from "../lib/types";
import type { AgentTemplate } from "../lib/agentTemplates";
import AgentCard from "../components/AgentCard";
import AgentForm from "../components/AgentForm";
import SkillStore from "../components/SkillStore";

type PageTab = "agents" | "skills";

export default function AgentsPage() {
  const [, setSearchParams] = useSearchParams();

  const [activeTab, setActiveTab] = useState<PageTab>("agents");
  const { agents, loading, refresh } = useAgents();
  const [search, setSearch] = useState("");

  // Form modal state
  const [showForm, setShowForm] = useState(false);
  const [editAgent, setEditAgent] = useState<Agent | null>(null);
  const [formTemplate, setFormTemplate] = useState<AgentTemplate | null>(null);

  const filteredAgents = agents.filter(
    (a) =>
      a.name.toLowerCase().includes(search.toLowerCase()) ||
      a.role.toLowerCase().includes(search.toLowerCase()),
  );

  const handleEnter = (agent: Agent) => {
    setSearchParams({ agent: agent.id });
    window.location.href = `/?agent=${agent.id}`;
  };

  const handleEdit = (agent: Agent) => {
    setEditAgent(agent);
    setFormTemplate(null);
    setShowForm(true);
  };

  const handleCreate = () => {
    setEditAgent(null);
    setFormTemplate(null);
    setShowForm(true);
  };

  const handleFormSaved = () => {
    setShowForm(false);
    setEditAgent(null);
    setFormTemplate(null);
    refresh();
  };

  const handleFormCancel = () => {
    setShowForm(false);
    setEditAgent(null);
    setFormTemplate(null);
  };

  return (
    <div className="agents-page">
      {/* Top bar */}
      <div className="page-top-bar">
        <div className="page-top-bar-left">
          <LayoutGrid size={20} />
          <h2>Agent & Skill</h2>
          <div className="page-tabs">
            <button
              className={`page-tab ${activeTab === "agents" ? "active" : ""}`}
              onClick={() => setActiveTab("agents")}
            >
              Agent
            </button>
            <button
              className={`page-tab ${activeTab === "skills" ? "active" : ""}`}
              onClick={() => setActiveTab("skills")}
            >
              Skill
            </button>
          </div>
        </div>
        <div className="page-top-bar-right">
          <button className="btn-primary-pill" onClick={handleCreate}>
            <Plus size={16} /> 新建 Agent
          </button>
          <div className="page-search-box">
            <Search size={14} />
            <input
              placeholder="搜索 Agent..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
            />
          </div>
        </div>
      </div>

      {/* Agent tab */}
      {activeTab === "agents" && (
        <>
          {loading ? (
            <div className="empty-state">
              <p>加载中...</p>
            </div>
          ) : filteredAgents.length === 0 ? (
            <div className="empty-state">
              <p>{search ? "没有匹配的 Agent" : "还没有 Agent，点击上方按钮创建"}</p>
            </div>
          ) : (
            <div className="agents-grid">
              {filteredAgents.map((agent) => (
                <AgentCard
                  key={agent.id}
                  agent={agent}
                  onEnter={handleEnter}
                  onEdit={handleEdit}
                />
              ))}
            </div>
          )}
        </>
      )}

      {/* Skill tab */}
      {activeTab === "skills" && (
        <div className="tab-content">
          <SkillStore />
        </div>
      )}

      {/* Agent Form Modal */}
      {showForm && (
        <AgentForm
          agent={editAgent}
          initialTemplate={formTemplate}
          onSaved={handleFormSaved}
          onCancel={handleFormCancel}
        />
      )}
    </div>
  );
}
