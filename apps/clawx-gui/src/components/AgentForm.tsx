import { useState } from "react";
import { createAgent, updateAgent } from "../lib/api";
import type { Agent } from "../lib/types";

interface AgentFormProps {
  agent?: Agent | null;
  onSaved: () => void;
  onCancel: () => void;
}

export default function AgentForm({ agent, onSaved, onCancel }: AgentFormProps) {
  const isEdit = !!agent;
  const [name, setName] = useState(agent?.name ?? "");
  const [role, setRole] = useState(agent?.role ?? "");
  const [systemPrompt, setSystemPrompt] = useState(agent?.system_prompt ?? "");
  const [modelId, setModelId] = useState(agent?.model_id ?? "");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || !role.trim()) return;

    setSaving(true);
    setError(null);
    try {
      const data: Record<string, string | null> = {
        name: name.trim(),
        role: role.trim(),
        system_prompt: systemPrompt.trim() || "",
        model_id: modelId.trim() || null,
      };

      if (isEdit) {
        await updateAgent(agent.id, data);
      } else {
        await createAgent(data);
      }
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to save agent");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="modal-overlay" onClick={onCancel}>
      <div className="modal-content" onClick={(e) => e.stopPropagation()}>
        <h3 className="modal-title">{isEdit ? "Edit Agent" : "Create Agent"}</h3>
        <form onSubmit={handleSubmit} className="agent-form">
          {error && <p className="form-error">{error}</p>}
          <label className="form-label">
            Name *
            <input
              type="text"
              className="form-input"
              value={name}
              onChange={(e) => setName(e.target.value)}
              required
              aria-label="Agent name"
              placeholder="e.g. Research Assistant"
            />
          </label>
          <label className="form-label">
            Role *
            <input
              type="text"
              className="form-input"
              value={role}
              onChange={(e) => setRole(e.target.value)}
              required
              aria-label="Agent role"
              placeholder="e.g. researcher"
            />
          </label>
          <label className="form-label">
            System Prompt
            <textarea
              className="form-textarea"
              value={systemPrompt}
              onChange={(e) => setSystemPrompt(e.target.value)}
              aria-label="System prompt"
              placeholder="Instructions for the agent..."
              rows={4}
            />
          </label>
          <label className="form-label">
            Model ID
            <input
              type="text"
              className="form-input"
              value={modelId}
              onChange={(e) => setModelId(e.target.value)}
              aria-label="Model ID"
              placeholder="e.g. claude-sonnet-4-20250514"
            />
          </label>
          <div className="form-actions">
            <button
              type="button"
              className="btn-secondary"
              onClick={onCancel}
              disabled={saving}
            >
              Cancel
            </button>
            <button
              type="submit"
              className="btn-primary"
              disabled={saving || !name.trim() || !role.trim()}
            >
              {saving ? "Saving..." : isEdit ? "Save Changes" : "Create"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
