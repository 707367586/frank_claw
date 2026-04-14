import { useState } from "react";
import { AGENT_TEMPLATES, type AgentTemplate } from "../lib/agentTemplates";

interface AgentTemplateGridProps {
  onUseTemplate: (template: AgentTemplate) => void;
}

export default function AgentTemplateGrid({
  onUseTemplate,
}: AgentTemplateGridProps) {
  const [search, setSearch] = useState("");

  const filtered = AGENT_TEMPLATES.filter(
    (t) =>
      t.name.toLowerCase().includes(search.toLowerCase()) ||
      t.description.toLowerCase().includes(search.toLowerCase()) ||
      t.role.toLowerCase().includes(search.toLowerCase()),
  );

  return (
    <div className="template-grid-wrapper">
      <p className="template-grid-hint">
        选择一个模板快速创建 Agent，模板预设了角色、提示词和技能。
      </p>

      <div className="template-grid-search">
        <input
          type="text"
          className="form-input"
          placeholder="搜索模板..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="template-grid">
        {filtered.map((t) => (
          <div key={t.id} className="template-card">
            <div className="template-card-header">
              <span className="template-card-icon">{t.icon}</span>
              <div>
                <div className="template-card-name">{t.name}</div>
                <div className="template-card-desc">{t.description}</div>
              </div>
            </div>

            <div className="template-card-skills">
              {t.skills.map((skill) => (
                <span key={skill} className="skill-tag">
                  {skill}
                </span>
              ))}
            </div>

            <button
              className="btn-primary template-card-btn"
              onClick={() => onUseTemplate(t)}
            >
              使用模板
            </button>
          </div>
        ))}

        {filtered.length === 0 && (
          <p className="template-grid-empty">没有匹配的模板</p>
        )}
      </div>
    </div>
  );
}
