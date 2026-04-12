import { useState, useEffect, useCallback, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import { Plus, Trash2, Key, KeyRound } from "lucide-react";
import {
  listModels,
  createModel,
  deleteModel,
  getHealth,
  getStats,
} from "../lib/api";
import type { ModelProvider, SystemHealth, SystemStats } from "../lib/types";

const PROVIDER_TYPES: ModelProvider["provider_type"][] = [
  "anthropic",
  "openai",
  "zhipuai",
  "ollama",
  "custom",
];

const PROVIDER_TYPE_LABELS: Record<ModelProvider["provider_type"], string> = {
  anthropic: "Anthropic",
  openai: "OpenAI",
  zhipuai: "ZhipuAI",
  ollama: "Ollama",
  custom: "Custom",
};

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
}

function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400);
  const h = Math.floor((seconds % 86400) / 3600);
  const m = Math.floor((seconds % 3600) / 60);
  const parts: string[] = [];
  if (d > 0) parts.push(`${d}d`);
  if (h > 0) parts.push(`${h}h`);
  parts.push(`${m}m`);
  return parts.join(" ");
}

// ── Models Section ──

function ModelsSection() {
  const [models, setModels] = useState<ModelProvider[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showForm, setShowForm] = useState(false);
  const [mutationError, setMutationError] = useState<string | null>(null);

  // Create form state
  const [formName, setFormName] = useState("");
  const [formType, setFormType] = useState<ModelProvider["provider_type"]>("anthropic");
  const [formApiKey, setFormApiKey] = useState("");
  const [formBaseUrl, setFormBaseUrl] = useState("");
  const [formDefaultModel, setFormDefaultModel] = useState("");
  const [creating, setCreating] = useState(false);

  const loadModels = useCallback(async () => {
    try {
      setError(null);
      const data = await listModels();
      setModels(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load models");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadModels();
  }, [loadModels]);

  const handleCreate = useCallback(async () => {
    if (!formName.trim()) return;
    setCreating(true);
    setMutationError(null);
    try {
      const data: Record<string, unknown> = {
        name: formName.trim(),
        provider_type: formType,
        default_model: formDefaultModel.trim(),
      };
      if (formApiKey) data.api_key = formApiKey;
      if (formBaseUrl) data.base_url = formBaseUrl;
      await createModel(data as Partial<Omit<ModelProvider, "id" | "created_at">>);
      setFormName("");
      setFormType("anthropic");
      setFormApiKey("");
      setFormBaseUrl("");
      setFormDefaultModel("");
      setShowForm(false);
      await loadModels();
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to create model provider",
      );
    } finally {
      setCreating(false);
    }
  }, [formName, formType, formApiKey, formBaseUrl, formDefaultModel, loadModels]);

  const handleDelete = useCallback(
    async (model: ModelProvider) => {
      const confirmed = window.confirm(
        `Delete model provider "${model.name}"? This cannot be undone.`,
      );
      if (!confirmed) return;
      try {
        await deleteModel(model.id);
        await loadModels();
      } catch (err) {
        setMutationError(
          err instanceof Error ? err.message : "Failed to delete model provider",
        );
      }
    },
    [loadModels],
  );

  const showBaseUrl = formType === "custom" || formType === "ollama";

  return (
    <div className="settings-section">
      <div className="settings-section-header">
        <h3 className="settings-section-title">Model Providers</h3>
        <button
          className="btn-primary"
          onClick={() => setShowForm((v) => !v)}
          aria-label="Add model provider"
        >
          <Plus size={14} />
          <span>Add Provider</span>
        </button>
      </div>

      {showForm && (
        <div className="settings-form-card">
          <h4 className="settings-form-title">New Model Provider</h4>
          <div className="settings-form">
            <label className="form-label">
              Name
              <input
                type="text"
                className="form-input"
                value={formName}
                onChange={(e) => setFormName(e.target.value)}
                placeholder="e.g. My OpenAI Key"
                aria-label="Provider name"
              />
            </label>
            <label className="form-label">
              Provider Type
              <select
                className="form-input"
                value={formType}
                onChange={(e) =>
                  setFormType(e.target.value as ModelProvider["provider_type"])
                }
                aria-label="Provider type"
              >
                {PROVIDER_TYPES.map((t) => (
                  <option key={t} value={t}>
                    {PROVIDER_TYPE_LABELS[t]}
                  </option>
                ))}
              </select>
            </label>
            <label className="form-label">
              API Key
              <input
                type="password"
                className="form-input"
                value={formApiKey}
                onChange={(e) => setFormApiKey(e.target.value)}
                placeholder="sk-..."
                aria-label="API key"
              />
            </label>
            {showBaseUrl && (
              <label className="form-label">
                Base URL
                <input
                  type="text"
                  className="form-input"
                  value={formBaseUrl}
                  onChange={(e) => setFormBaseUrl(e.target.value)}
                  placeholder="http://localhost:11434"
                  aria-label="Base URL"
                />
              </label>
            )}
            <label className="form-label">
              Default Model
              <input
                type="text"
                className="form-input"
                value={formDefaultModel}
                onChange={(e) => setFormDefaultModel(e.target.value)}
                placeholder="e.g. gpt-4o, claude-sonnet-4-20250514"
                aria-label="Default model"
              />
            </label>
            {mutationError && <p className="form-error">{mutationError}</p>}
            <div className="form-actions">
              <button
                className="btn-secondary"
                onClick={() => setShowForm(false)}
                aria-label="Cancel"
              >
                Cancel
              </button>
              <button
                className="btn-primary"
                onClick={handleCreate}
                disabled={creating || !formName.trim()}
                aria-label="Create model provider"
              >
                {creating ? "Creating..." : "Create"}
              </button>
            </div>
          </div>
        </div>
      )}

      {loading && <p className="list-placeholder">Loading models...</p>}
      {error && <p className="form-error">{error}</p>}

      {!loading && !error && models.length === 0 && (
        <p className="list-placeholder">No model providers configured yet.</p>
      )}

      <div className="model-provider-list">
        {models.map((model) => (
          <div key={model.id} className="model-provider-card">
            <div className="model-provider-card-top">
              <div className="model-provider-card-info">
                <span className="model-provider-name">{model.name}</span>
                <span className="model-provider-type-badge">
                  {PROVIDER_TYPE_LABELS[model.provider_type]}
                </span>
              </div>
              <button
                className="btn-icon-sm btn-danger"
                onClick={() => handleDelete(model)}
                title="Delete provider"
                aria-label={`Delete model provider ${model.name}`}
              >
                <Trash2 size={14} />
              </button>
            </div>
            <div className="model-provider-details">
              {model.base_url && (
                <div className="model-provider-detail-row">
                  <span className="model-provider-detail-label">URL</span>
                  <span className="model-provider-detail-value">{model.base_url}</span>
                </div>
              )}
              <div className="model-provider-detail-row">
                <span className="model-provider-detail-label">Model</span>
                <span className="model-provider-detail-value">
                  {model.default_model || "—"}
                </span>
              </div>
              <div className="model-provider-detail-row">
                <span className="model-provider-detail-label">API Key</span>
                <span className="model-provider-detail-value">
                  {model.api_key_set ? (
                    <span className="api-key-set">
                      <Key size={12} /> Set
                    </span>
                  ) : (
                    <span className="api-key-not-set">
                      <KeyRound size={12} /> Not set
                    </span>
                  )}
                </span>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ── Security Section ──

function SecuritySection() {
  return (
    <div className="settings-section">
      <h3 className="settings-section-title">Security</h3>
      <div className="settings-placeholder-list">
        <div className="settings-placeholder-card">
          <h4 className="settings-placeholder-title">Network Whitelist</h4>
          <p className="settings-placeholder-text">
            Network whitelist management — coming in future update
          </p>
        </div>
        <div className="settings-placeholder-card">
          <h4 className="settings-placeholder-title">DLP (Data Loss Prevention)</h4>
          <p className="settings-placeholder-text">
            DLP policy status — coming in future update
          </p>
        </div>
        <div className="settings-placeholder-card">
          <h4 className="settings-placeholder-title">Prompt Injection Defense</h4>
          <p className="settings-placeholder-text">
            Prompt injection defense status — coming in future update
          </p>
        </div>
      </div>
    </div>
  );
}

// ── System Section ──

function SystemSection() {
  const [health, setHealth] = useState<SystemHealth | null>(null);
  const [stats, setStats] = useState<SystemStats | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const intervalRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const loadData = useCallback(async () => {
    try {
      setError(null);
      const [h, s] = await Promise.all([getHealth(), getStats()]);
      setHealth(h);
      setStats(s);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load system data");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
    intervalRef.current = setInterval(loadData, 30_000);
    return () => {
      if (intervalRef.current) clearInterval(intervalRef.current);
    };
  }, [loadData]);

  const statusColor =
    health?.status === "ok" || health?.status === "healthy"
      ? "#4ade80"
      : health?.status === "degraded"
        ? "#facc15"
        : "#f87171";

  return (
    <div className="settings-section">
      <h3 className="settings-section-title">System</h3>

      {loading && <p className="list-placeholder">Loading system data...</p>}
      {error && <p className="form-error">{error}</p>}

      {health && (
        <div className="system-health-card">
          <h4 className="system-card-title">Health</h4>
          <div className="system-health-grid">
            <div className="system-stat-card">
              <span className="system-stat-label">Status</span>
              <span className="system-stat-value">
                <span
                  className="system-status-dot"
                  style={{ background: statusColor }}
                />
                {health.status}
              </span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-label">Uptime</span>
              <span className="system-stat-value">{formatUptime(health.uptime)}</span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-label">Version</span>
              <span className="system-stat-value">{health.version}</span>
            </div>
          </div>
        </div>
      )}

      {stats && (
        <div className="system-stats-card">
          <h4 className="system-card-title">Statistics</h4>
          <div className="system-stats-grid">
            <div className="system-stat-card">
              <span className="system-stat-value">{stats.agent_count}</span>
              <span className="system-stat-label">Agents</span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-value">{stats.conversation_count}</span>
              <span className="system-stat-label">Conversations</span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-value">{stats.memory_count}</span>
              <span className="system-stat-label">Memories</span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-value">{stats.knowledge_doc_count}</span>
              <span className="system-stat-label">Knowledge Docs</span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-value">
                {formatBytes(stats.disk_usage_bytes)}
              </span>
              <span className="system-stat-label">Disk Usage</span>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

// ── About Section ──

function AboutSection() {
  return (
    <div className="settings-section">
      <h3 className="settings-section-title">About</h3>
      <div className="about-card">
        <div className="about-logo">CX</div>
        <h4 className="about-app-name">ClawX</h4>
        <p className="about-version">Version 0.2.0</p>
        <div className="about-details">
          <div className="about-detail-row">
            <span className="about-detail-label">Architecture</span>
            <span className="about-detail-value">Rust + React + TypeScript</span>
          </div>
          <div className="about-detail-row">
            <span className="about-detail-label">Frontend</span>
            <span className="about-detail-value">Vite + React 19</span>
          </div>
          <div className="about-detail-row">
            <span className="about-detail-label">Backend</span>
            <span className="about-detail-value">Rust (Axum)</span>
          </div>
        </div>
        <p className="about-links-placeholder">Links — coming in future update</p>
      </div>
    </div>
  );
}

// ── Main Settings Page ──

export default function SettingsPage() {
  const [searchParams] = useSearchParams();
  const section = searchParams.get("section") ?? "models";

  return (
    <div className="settings-page">
      {section === "models" && <ModelsSection />}
      {section === "security" && <SecuritySection />}
      {section === "system" && <SystemSection />}
      {section === "about" && <AboutSection />}
    </div>
  );
}
