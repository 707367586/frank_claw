import { useState, useEffect, useCallback, useRef } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Plus,
  Trash2,
  Key,
  FlaskConical,
  Pencil,
  Shield,
  FileWarning,
  Network,
  Sun,
  Moon,
  Monitor,
  Send,
} from "lucide-react";
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
  "zhipu",
  "ollama",
  "custom",
];

const PROVIDER_TYPE_LABELS: Record<ModelProvider["provider_type"], string> = {
  anthropic: "Anthropic (Claude)",
  openai: "OpenAI (GPT)",
  zhipu: "ZhipuAI (GLM)",
  ollama: "Ollama (Local)",
  custom: "Custom",
};

const PROVIDER_SHORT_LABELS: Record<ModelProvider["provider_type"], string> = {
  anthropic: "AN",
  openai: "OA",
  zhipu: "ZP",
  ollama: "OL",
  custom: "CU",
};

function getDefaultBaseUrl(providerType: string): string {
  switch (providerType) {
    case "anthropic": return "https://api.anthropic.com";
    case "openai": return "https://api.openai.com/v1";
    case "zhipu": return "https://open.bigmodel.cn/api/paas/v4";
    default: return "http://localhost:11434";
  }
}

const PROVIDER_COLORS: Record<ModelProvider["provider_type"], string> = {
  anthropic: "#d97706",
  openai: "#10b981",
  zhipu: "#6366f1",
  ollama: "#8b5cf6",
  custom: "#64748b",
};

// Mock data for agent model assignment
const MOCK_AGENT_ASSIGNMENTS = [
  {
    id: "1",
    agent: "客服助手",
    strategy: "Fixed" as const,
    model: "claude-sonnet-4-20250514",
    provider: "Anthropic",
  },
  {
    id: "2",
    agent: "代码审查",
    strategy: "Smart Routing" as const,
    model: "gpt-4o / claude-sonnet-4-20250514",
    provider: "Multi",
  },
  {
    id: "3",
    agent: "数据分析",
    strategy: "Fixed" as const,
    model: "glm-4",
    provider: "ZhipuAI",
  },
];

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
  const [testingId, setTestingId] = useState<string | null>(null);

  // Create form state
  const [formName, setFormName] = useState("");
  const [formType, setFormType] =
    useState<ModelProvider["provider_type"]>("anthropic");
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
        model_name: formDefaultModel.trim(),
        base_url: formBaseUrl || getDefaultBaseUrl(formType),
      };
      await createModel(
        data as Partial<Omit<ModelProvider, "id" | "created_at">>,
      );
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
  }, [
    formName,
    formType,
    formApiKey,
    formBaseUrl,
    formDefaultModel,
    loadModels,
  ]);

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
          err instanceof Error
            ? err.message
            : "Failed to delete model provider",
        );
      }
    },
    [loadModels],
  );

  const handleTestConnection = useCallback(async (modelId: string) => {
    setTestingId(modelId);
    // Simulate test connection delay
    setTimeout(() => {
      setTestingId(null);
    }, 2000);
  }, []);

  const showBaseUrl = formType === "custom" || formType === "ollama";

  return (
    <div className="settings-section">
      {/* Provider cards */}
      <div className="settings-section-header">
        <h3 className="settings-section-title">Model Provider</h3>
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
                  setFormType(
                    e.target.value as ModelProvider["provider_type"],
                  )
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

      <div className="provider-card-list">
        {models.map((model) => {
          const isAvailable = !!model.model_name;
          return (
            <div key={model.id} className="provider-card">
              <div className="provider-card-header">
                <div className="provider-card-identity">
                  <div
                    className="provider-avatar"
                    style={{
                      background: PROVIDER_COLORS[model.provider_type],
                    }}
                  >
                    {PROVIDER_SHORT_LABELS[model.provider_type]}
                  </div>
                  <div className="provider-card-name-group">
                    <span className="provider-card-name">{model.name}</span>
                    <span className="provider-card-type">
                      {PROVIDER_TYPE_LABELS[model.provider_type]}
                    </span>
                  </div>
                </div>
                <span
                  className={`provider-status-badge ${isAvailable ? "status-available" : "status-needs-config"}`}
                >
                  {isAvailable ? "Available" : "需配置"}
                </span>
              </div>

              <div className="provider-card-details">
                <div className="provider-detail-row">
                  <span className="provider-detail-label">Models</span>
                  <span className="provider-detail-value">
                    {model.model_name || "—"}
                  </span>
                </div>
                <div className="provider-detail-row">
                  <span className="provider-detail-label">API Key</span>
                  <span className="provider-detail-value">
                    <span className="api-key-set">
                      <Key size={12} /> Configured via ENV
                    </span>
                  </span>
                </div>
                {model.base_url && (
                  <div className="provider-detail-row">
                    <span className="provider-detail-label">Base URL</span>
                    <span className="provider-detail-value">
                      {model.base_url}
                    </span>
                  </div>
                )}
              </div>

              <div className="provider-actions">
                <button
                  className="btn-secondary btn-sm"
                  onClick={() => handleTestConnection(model.id)}
                  disabled={testingId === model.id}
                >
                  <FlaskConical size={13} />
                  <span>
                    {testingId === model.id
                      ? "Testing..."
                      : "Test Connection"}
                  </span>
                </button>
                <button className="btn-secondary btn-sm">
                  <Pencil size={13} />
                  <span>Edit</span>
                </button>
                <button
                  className="btn-icon-sm btn-danger"
                  onClick={() => handleDelete(model)}
                  title="Delete provider"
                  aria-label={`Delete model provider ${model.name}`}
                >
                  <Trash2 size={14} />
                </button>
              </div>
            </div>
          );
        })}
      </div>

      {/* Agent Model Assignment */}
      <div className="settings-section-header" style={{ marginTop: 24 }}>
        <h3 className="settings-section-title">Agent Model Assignment</h3>
      </div>

      <div className="model-assignment-table-wrap">
        <table className="model-assignment-table">
          <thead>
            <tr>
              <th>Agent</th>
              <th>Strategy</th>
              <th>Model</th>
              <th>Action</th>
            </tr>
          </thead>
          <tbody>
            {MOCK_AGENT_ASSIGNMENTS.map((row) => (
              <tr key={row.id}>
                <td className="assignment-agent">{row.agent}</td>
                <td>
                  <span
                    className={`assignment-strategy ${row.strategy === "Smart Routing" ? "strategy-smart" : ""}`}
                  >
                    {row.strategy}
                  </span>
                </td>
                <td className="assignment-model">{row.model}</td>
                <td>
                  <button className="btn-link btn-sm">编辑</button>
                </td>
              </tr>
            ))}
            {MOCK_AGENT_ASSIGNMENTS.length === 0 && (
              <tr>
                <td colSpan={4} className="assignment-empty">
                  No agent assignments yet.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ── Security Section ──

function SecuritySection() {
  return (
    <div className="settings-section">
      <h3 className="settings-section-title">安全</h3>

      <div className="security-categories">
        <div className="security-card">
          <div className="security-card-header">
            <Network size={18} className="security-card-icon" />
            <div>
              <h4 className="security-card-title">Network Whitelist</h4>
              <p className="security-card-desc">
                配置允许访问的 IP 地址和域名白名单，限制 Agent 的网络访问范围
              </p>
            </div>
          </div>
          <div className="security-card-body">
            <div className="security-rule-list">
              <div className="security-rule-item">
                <span className="security-rule-label">允许的域名</span>
                <span className="security-rule-value">*.anthropic.com, *.openai.com</span>
              </div>
              <div className="security-rule-item">
                <span className="security-rule-label">允许的 IP</span>
                <span className="security-rule-value security-placeholder-value">未配置</span>
              </div>
              <div className="security-rule-item">
                <span className="security-rule-label">状态</span>
                <span className="security-rule-value">
                  <span className="provider-status-badge status-needs-config">未启用</span>
                </span>
              </div>
            </div>
          </div>
        </div>

        <div className="security-card">
          <div className="security-card-header">
            <FileWarning size={18} className="security-card-icon" />
            <div>
              <h4 className="security-card-title">DLP (Data Loss Prevention)</h4>
              <p className="security-card-desc">
                防止敏感数据（API Key、密码、PII）通过 Agent 对话泄露
              </p>
            </div>
          </div>
          <div className="security-card-body">
            <div className="security-rule-list">
              <div className="security-rule-item">
                <span className="security-rule-label">规则数量</span>
                <span className="security-rule-value">3 条内置规则</span>
              </div>
              <div className="security-rule-item">
                <span className="security-rule-label">自定义规则</span>
                <span className="security-rule-value security-placeholder-value">未配置</span>
              </div>
              <div className="security-rule-item">
                <span className="security-rule-label">状态</span>
                <span className="security-rule-value">
                  <span className="provider-status-badge status-needs-config">未启用</span>
                </span>
              </div>
            </div>
          </div>
        </div>

        <div className="security-card">
          <div className="security-card-header">
            <Shield size={18} className="security-card-icon" />
            <div>
              <h4 className="security-card-title">Prompt Injection Defense</h4>
              <p className="security-card-desc">
                检测并阻止恶意 Prompt 注入攻击，保护 Agent 执行安全
              </p>
            </div>
          </div>
          <div className="security-card-body">
            <div className="security-rule-list">
              <div className="security-rule-item">
                <span className="security-rule-label">检测模式</span>
                <span className="security-rule-value">基于规则 + LLM 双重检测</span>
              </div>
              <div className="security-rule-item">
                <span className="security-rule-label">拦截策略</span>
                <span className="security-rule-value security-placeholder-value">未配置</span>
              </div>
              <div className="security-rule-item">
                <span className="security-rule-label">状态</span>
                <span className="security-rule-value">
                  <span className="provider-status-badge status-needs-config">未启用</span>
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── Appearance & Language Section ──

function AppearanceSection() {
  const [theme, setTheme] = useState<"dark" | "light" | "system">("dark");
  const [language, setLanguage] = useState<"zh" | "en">("zh");

  return (
    <div className="settings-section">
      <h3 className="settings-section-title">外观与语言</h3>

      <div className="appearance-section">
        <div className="appearance-card">
          <h4 className="appearance-card-title">主题</h4>
          <p className="appearance-card-desc">选择界面主题配色方案</p>
          <div className="theme-selector">
            <button
              className={`theme-option ${theme === "dark" ? "selected" : ""}`}
              onClick={() => setTheme("dark")}
            >
              <Moon size={16} />
              <span>暗色</span>
            </button>
            <button
              className={`theme-option ${theme === "light" ? "selected" : ""}`}
              onClick={() => setTheme("light")}
            >
              <Sun size={16} />
              <span>亮色</span>
            </button>
            <button
              className={`theme-option ${theme === "system" ? "selected" : ""}`}
              onClick={() => setTheme("system")}
            >
              <Monitor size={16} />
              <span>跟随系统</span>
            </button>
          </div>
        </div>

        <div className="appearance-card">
          <h4 className="appearance-card-title">语言</h4>
          <p className="appearance-card-desc">选择界面显示语言</p>
          <div className="language-selector">
            <button
              className={`language-option ${language === "zh" ? "selected" : ""}`}
              onClick={() => setLanguage("zh")}
            >
              中文
            </button>
            <button
              className={`language-option ${language === "en" ? "selected" : ""}`}
              onClick={() => setLanguage("en")}
            >
              English
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

// ── System / Health Section ──

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
      setError(
        err instanceof Error ? err.message : "Failed to load system data",
      );
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
      <h3 className="settings-section-title">系统健康</h3>

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
              <span className="system-stat-value">
                {formatUptime(health.uptime)}
              </span>
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
              <span className="system-stat-value">
                {stats.conversation_count}
              </span>
              <span className="system-stat-label">Conversations</span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-value">{stats.memory_count}</span>
              <span className="system-stat-label">Memories</span>
            </div>
            <div className="system-stat-card">
              <span className="system-stat-value">
                {stats.knowledge_doc_count}
              </span>
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
      <h3 className="settings-section-title">关于</h3>
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
      </div>
    </div>
  );
}

// ── Feedback Section ──

function FeedbackSection() {
  const [feedbackText, setFeedbackText] = useState("");
  const [submitted, setSubmitted] = useState(false);

  const handleSubmit = useCallback(() => {
    if (!feedbackText.trim()) return;
    // Simulate submit
    setSubmitted(true);
    setFeedbackText("");
    setTimeout(() => setSubmitted(false), 3000);
  }, [feedbackText]);

  return (
    <div className="settings-section">
      <h3 className="settings-section-title">反馈</h3>

      <div className="feedback-section">
        <div className="feedback-card">
          <p className="feedback-desc">
            您的反馈对我们非常重要。请描述您遇到的问题或建议，我们会尽快处理。
          </p>
          <textarea
            className="feedback-textarea"
            value={feedbackText}
            onChange={(e) => setFeedbackText(e.target.value)}
            placeholder="请输入您的反馈内容..."
            rows={5}
          />
          <div className="feedback-actions">
            {submitted && (
              <span className="feedback-success">感谢您的反馈！</span>
            )}
            <button
              className="btn-primary"
              onClick={handleSubmit}
              disabled={!feedbackText.trim()}
            >
              <Send size={14} />
              <span>发送反馈</span>
            </button>
          </div>
          <a
            className="feedback-link"
            href="https://github.com"
            target="_blank"
            rel="noopener noreferrer"
          >
            或在 GitHub 上提交 Issue
          </a>
        </div>
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
      {section === "appearance" && <AppearanceSection />}
      {section === "system" && <SystemSection />}
      {section === "about" && <AboutSection />}
      {section === "feedback" && <FeedbackSection />}
    </div>
  );
}
