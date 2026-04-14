import { useEffect, useState } from "react";
import { createAgent, listModels, updateAgent } from "../lib/api";
import { AGENT_TEMPLATES, type AgentTemplate } from "../lib/agentTemplates";
import type { Agent, ModelProvider } from "../lib/types";

interface AgentFormProps {
  agent?: Agent | null;
  /** Pre-select a template when opening in create mode */
  initialTemplate?: AgentTemplate | null;
  onSaved: () => void;
  onCancel: () => void;
}

type TabMode = "template" | "custom";

export default function AgentForm({
  agent,
  initialTemplate,
  onSaved,
  onCancel,
}: AgentFormProps) {
  const isEdit = !!agent;

  // Tab state
  const [tab, setTab] = useState<TabMode>(
    initialTemplate ? "template" : isEdit ? "custom" : "template",
  );

  // Template tab state
  const [selectedTemplate, setSelectedTemplate] = useState<AgentTemplate | null>(
    initialTemplate ?? null,
  );
  const [templateName, setTemplateName] = useState(initialTemplate?.name ?? "");

  // Custom tab state
  const [name, setName] = useState(agent?.name ?? "");
  const [description, setDescription] = useState("");
  const [role, setRole] = useState(agent?.role ?? "");
  const [systemPrompt, setSystemPrompt] = useState(agent?.system_prompt ?? "");
  const [avatarPreview, setAvatarPreview] = useState<string | null>(null);

  // Shared state
  const [modelId, setModelId] = useState(agent?.model_id ?? "");
  const [models, setModels] = useState<ModelProvider[]>([]);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    listModels()
      .then((list) => {
        setModels(list);
        // Auto-select default model if none set
        if (!modelId && list.length > 0) {
          const defaultModel = list.find((m) => m.is_default) ?? list[0];
          setModelId(defaultModel.id);
        }
      })
      .catch(() => {});
  }, []);

  // When template selection changes, update template name default
  const handleSelectTemplate = (t: AgentTemplate) => {
    setSelectedTemplate(t);
    if (!templateName || templateName === selectedTemplate?.name) {
      setTemplateName(t.name);
    }
  };

  const handleAvatarChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      const reader = new FileReader();
      reader.onload = () => setAvatarPreview(reader.result as string);
      reader.readAsDataURL(file);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSaving(true);
    setError(null);

    try {
      if (isEdit) {
        await updateAgent(agent.id, {
          name: name.trim(),
          role: role.trim(),
          system_prompt: systemPrompt.trim(),
          model_id: modelId || null,
        });
      } else if (tab === "template" && selectedTemplate) {
        await createAgent({
          name: templateName.trim() || selectedTemplate.name,
          role: selectedTemplate.role,
          system_prompt: selectedTemplate.systemPrompt,
          model_id: modelId || null,
        });
      } else {
        await createAgent({
          name: name.trim(),
          role: role.trim() || "Assistant",
          system_prompt: systemPrompt.trim(),
          model_id: modelId || null,
        });
      }
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save agent");
    } finally {
      setSaving(false);
    }
  };

  const canSubmitTemplate = !!selectedTemplate;
  const canSubmitCustom = !!name.trim();
  const canSubmit = isEdit
    ? canSubmitCustom
    : tab === "template"
      ? canSubmitTemplate
      : canSubmitCustom;

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div
        className="modal-content agent-form-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <h3 className="modal-title">
          {isEdit ? "编辑 Agent" : "创建新 Agent"}
        </h3>

        {/* Tabs — only show in create mode */}
        {!isEdit && (
          <div className="agent-form-tabs">
            <button
              type="button"
              className={`agent-form-tab ${tab === "template" ? "active" : ""}`}
              onClick={() => setTab("template")}
            >
              从模板
            </button>
            <button
              type="button"
              className={`agent-form-tab ${tab === "custom" ? "active" : ""}`}
              onClick={() => setTab("custom")}
            >
              自定义
            </button>
          </div>
        )}

        <form onSubmit={handleSubmit} className="agent-form">
          {error && <p className="form-error">{error}</p>}

          {/* ── Template Tab ── */}
          {!isEdit && tab === "template" && (
            <>
              <div className="agent-form-template-list">
                {AGENT_TEMPLATES.map((t) => (
                  <div
                    key={t.id}
                    className={`agent-form-template-card ${selectedTemplate?.id === t.id ? "selected" : ""}`}
                    onClick={() => handleSelectTemplate(t)}
                  >
                    <span className="agent-form-template-icon">{t.icon}</span>
                    <div className="agent-form-template-info">
                      <span className="agent-form-template-name">{t.name}</span>
                      <span className="agent-form-template-desc">
                        {t.description}
                      </span>
                    </div>
                  </div>
                ))}
              </div>

              <label className="form-label">
                Agent 名称
                <input
                  type="text"
                  className="form-input"
                  value={templateName}
                  onChange={(e) => setTemplateName(e.target.value)}
                  placeholder={selectedTemplate?.name ?? "输入 Agent 名称"}
                />
              </label>

              <label className="form-label">
                模型
                <select
                  className="form-input"
                  value={modelId}
                  onChange={(e) => setModelId(e.target.value)}
                >
                  <option value="">默认模型</option>
                  {models.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name} ({m.model_name})
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}

          {/* ── Custom Tab (or Edit mode) ── */}
          {(isEdit || tab === "custom") && (
            <>
              {!isEdit && (
                <div className="agent-form-avatar-area">
                  <label className="agent-form-avatar-upload">
                    {avatarPreview ? (
                      <img
                        src={avatarPreview}
                        alt="avatar"
                        className="agent-form-avatar-img"
                      />
                    ) : (
                      <div className="agent-form-avatar-placeholder">
                        <span>+</span>
                        <span className="agent-form-avatar-hint">上传头像</span>
                      </div>
                    )}
                    <input
                      type="file"
                      accept="image/*"
                      onChange={handleAvatarChange}
                      style={{ display: "none" }}
                    />
                  </label>
                </div>
              )}

              <label className="form-label">
                Agent 名称 *
                <input
                  type="text"
                  className="form-input"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  required
                  placeholder="例如：研究助手"
                />
              </label>

              {!isEdit && (
                <label className="form-label">
                  描述
                  <input
                    type="text"
                    className="form-input"
                    value={description}
                    onChange={(e) => setDescription(e.target.value)}
                    placeholder="简短描述这个 Agent 的用途"
                  />
                </label>
              )}

              {isEdit && (
                <label className="form-label">
                  角色
                  <input
                    type="text"
                    className="form-input"
                    value={role}
                    onChange={(e) => setRole(e.target.value)}
                    placeholder="例如：Researcher"
                  />
                </label>
              )}

              <label className="form-label">
                System Prompt
                <textarea
                  className="form-textarea"
                  value={systemPrompt}
                  onChange={(e) => setSystemPrompt(e.target.value)}
                  placeholder="为 Agent 编写系统提示词..."
                  rows={4}
                />
              </label>

              <label className="form-label">
                模型
                <select
                  className="form-input"
                  value={modelId}
                  onChange={(e) => setModelId(e.target.value)}
                >
                  <option value="">默认模型</option>
                  {models.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name} ({m.model_name})
                    </option>
                  ))}
                </select>
              </label>
            </>
          )}

          <div className="form-actions">
            <button
              type="button"
              className="btn-secondary"
              onClick={onCancel}
              disabled={saving}
            >
              取消
            </button>
            <button
              type="submit"
              className="btn-primary"
              disabled={saving || !canSubmit}
            >
              {saving ? "保存中..." : isEdit ? "保存" : "创建"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
