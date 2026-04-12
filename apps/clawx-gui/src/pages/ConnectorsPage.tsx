import { useState, useEffect, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import { Trash2 } from "lucide-react";
import {
  getChannel,
  updateChannel,
  deleteChannel,
  connectChannel,
  disconnectChannel,
  createChannel,
  listAgents,
} from "../lib/api";
import type { Channel, Agent } from "../lib/types";

const CHANNEL_STATUS_COLORS: Record<Channel["status"], string> = {
  connected: "#4ade80",
  disconnected: "#6b7280",
  error: "#f87171",
};

const CHANNEL_TYPES: Channel["channel_type"][] = [
  "telegram",
  "lark",
  "slack",
  "discord",
  "whatsapp",
  "wecom",
];

const CHANNEL_TYPE_DISPLAY: Record<Channel["channel_type"], string> = {
  telegram: "Telegram",
  lark: "Lark",
  slack: "Slack",
  discord: "Discord",
  whatsapp: "WhatsApp",
  wecom: "WeChat Enterprise",
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

export default function ConnectorsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const channelId = searchParams.get("channel");
  const addMode = searchParams.get("add") === "1";

  const [channel, setChannel] = useState<Channel | null>(null);
  const [agents, setAgents] = useState<Agent[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [toggling, setToggling] = useState(false);

  // Edit form state
  const [editName, setEditName] = useState("");
  const [editAgentId, setEditAgentId] = useState("");
  const [editConfig, setEditConfig] = useState<Record<string, string>>({});
  const [editConfigJson, setEditConfigJson] = useState("");

  // Create form state
  const [createType, setCreateType] = useState<Channel["channel_type"] | null>(
    null,
  );
  const [createName, setCreateName] = useState("");
  const [createAgentId, setCreateAgentId] = useState("");
  const [createConfig, setCreateConfig] = useState<Record<string, string>>({});
  const [createConfigJson, setCreateConfigJson] = useState("{}");
  const [creating, setCreating] = useState(false);

  useEffect(() => {
    listAgents()
      .then(setAgents)
      .catch(() => {
        /* agent list is optional */
      });
  }, []);

  const loadChannel = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const data = await getChannel(id);
      setChannel(data);
      setEditName(data.name);
      setEditAgentId(data.agent_id);
      if (data.channel_type === "telegram") {
        setEditConfig({
          bot_token: (data.config.bot_token as string) ?? "",
        });
      } else if (data.channel_type === "lark") {
        setEditConfig({
          app_id: (data.config.app_id as string) ?? "",
          app_secret: (data.config.app_secret as string) ?? "",
        });
      } else {
        setEditConfigJson(JSON.stringify(data.config, null, 2));
      }
    } catch (err) {
      setError(
        err instanceof Error ? err.message : "Failed to load channel",
      );
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    if (!channelId) {
      setChannel(null);
      return;
    }
    loadChannel(channelId);
  }, [channelId, loadChannel]);

  const handleSave = useCallback(async () => {
    if (!channel) return;
    setSaving(true);
    setMutationError(null);
    try {
      let config: Record<string, unknown>;
      if (
        channel.channel_type === "telegram" ||
        channel.channel_type === "lark"
      ) {
        config = { ...editConfig };
      } else {
        config = JSON.parse(editConfigJson);
      }
      const updated = await updateChannel(channel.id, {
        name: editName,
        agent_id: editAgentId,
        config,
      });
      setChannel(updated);
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to save channel",
      );
    } finally {
      setSaving(false);
    }
  }, [channel, editName, editAgentId, editConfig, editConfigJson]);

  const handleToggleConnection = useCallback(async () => {
    if (!channel) return;
    setToggling(true);
    setMutationError(null);
    try {
      const updated =
        channel.status === "connected"
          ? await disconnectChannel(channel.id)
          : await connectChannel(channel.id);
      setChannel(updated);
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to toggle connection",
      );
    } finally {
      setToggling(false);
    }
  }, [channel]);

  const handleDelete = useCallback(async () => {
    if (!channel) return;
    const confirmed = window.confirm(
      `Delete channel "${channel.name}"? This cannot be undone.`,
    );
    if (!confirmed) return;
    setMutationError(null);
    try {
      await deleteChannel(channel.id);
      setChannel(null);
      setSearchParams({});
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to delete channel",
      );
    }
  }, [channel, setSearchParams]);

  const handleCreate = useCallback(async () => {
    if (!createType || !createName.trim()) return;
    setCreating(true);
    setMutationError(null);
    try {
      let config: Record<string, unknown>;
      if (createType === "telegram" || createType === "lark") {
        config = { ...createConfig };
      } else {
        config = JSON.parse(createConfigJson);
      }
      const created = await createChannel({
        name: createName.trim(),
        channel_type: createType,
        agent_id: createAgentId,
        config,
        status: "disconnected",
      });
      setSearchParams({ channel: created.id });
      setCreateType(null);
      setCreateName("");
      setCreateAgentId("");
      setCreateConfig({});
      setCreateConfigJson("{}");
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to create channel",
      );
    } finally {
      setCreating(false);
    }
  }, [
    createType,
    createName,
    createAgentId,
    createConfig,
    createConfigJson,
    setSearchParams,
  ]);

  // Add Channel mode (no channel selected)
  if (addMode && !channelId) {
    return (
      <div className="channel-page">
        <div className="channel-create-section">
          <h3 className="channel-section-title">Add Channel</h3>
          {!createType ? (
            <div className="channel-type-grid">
              {CHANNEL_TYPES.map((type) => (
                <button
                  key={type}
                  className="channel-type-card"
                  onClick={() => setCreateType(type)}
                  aria-label={`Select channel type ${CHANNEL_TYPE_DISPLAY[type]}`}
                >
                  <span className="channel-type-card-badge">{type}</span>
                  <span className="channel-type-card-name">
                    {CHANNEL_TYPE_DISPLAY[type]}
                  </span>
                </button>
              ))}
            </div>
          ) : (
            <div className="channel-form">
              <div className="channel-form-type-header">
                <span className="channel-type-badge-lg">{createType}</span>
                <span>{CHANNEL_TYPE_DISPLAY[createType]}</span>
                <button
                  className="btn-secondary"
                  onClick={() => setCreateType(null)}
                  aria-label="Change channel type"
                >
                  Change
                </button>
              </div>

              <label className="form-label">
                Name
                <input
                  type="text"
                  className="form-input"
                  value={createName}
                  onChange={(e) => setCreateName(e.target.value)}
                  placeholder="Channel name"
                  aria-label="Channel name"
                />
              </label>

              <label className="form-label">
                Bound Agent
                <select
                  className="form-input"
                  value={createAgentId}
                  onChange={(e) => setCreateAgentId(e.target.value)}
                  aria-label="Select agent"
                >
                  <option value="">Select agent...</option>
                  {agents.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name}
                    </option>
                  ))}
                </select>
              </label>

              {createType === "telegram" && (
                <label className="form-label">
                  Bot Token
                  <input
                    type="text"
                    className="form-input"
                    value={createConfig.bot_token ?? ""}
                    onChange={(e) =>
                      setCreateConfig((c) => ({
                        ...c,
                        bot_token: e.target.value,
                      }))
                    }
                    placeholder="123456:ABC-DEF..."
                    aria-label="Telegram bot token"
                  />
                </label>
              )}

              {createType === "lark" && (
                <>
                  <label className="form-label">
                    App ID
                    <input
                      type="text"
                      className="form-input"
                      value={createConfig.app_id ?? ""}
                      onChange={(e) =>
                        setCreateConfig((c) => ({
                          ...c,
                          app_id: e.target.value,
                        }))
                      }
                      placeholder="cli_xxxxx"
                      aria-label="Lark app ID"
                    />
                  </label>
                  <label className="form-label">
                    App Secret
                    <input
                      type="text"
                      className="form-input"
                      value={createConfig.app_secret ?? ""}
                      onChange={(e) =>
                        setCreateConfig((c) => ({
                          ...c,
                          app_secret: e.target.value,
                        }))
                      }
                      placeholder="App secret"
                      aria-label="Lark app secret"
                    />
                  </label>
                </>
              )}

              {createType !== "telegram" && createType !== "lark" && (
                <label className="form-label">
                  Configuration (JSON)
                  <textarea
                    className="form-textarea"
                    value={createConfigJson}
                    onChange={(e) => setCreateConfigJson(e.target.value)}
                    rows={6}
                    aria-label="Channel configuration JSON"
                  />
                </label>
              )}

              {mutationError && <p className="form-error">{mutationError}</p>}

              <div className="form-actions">
                <button
                  className="btn-primary"
                  onClick={handleCreate}
                  disabled={creating || !createName.trim() || !createType}
                  aria-label="Create channel"
                >
                  {creating ? "Creating..." : "Create Channel"}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    );
  }

  // No channel selected, no add mode
  if (!channelId) {
    return (
      <div className="empty-state">
        <h2>Connectors</h2>
        <p>Select a channel or add a new one.</p>
      </div>
    );
  }

  // Channel detail / edit view
  return (
    <div className="channel-page">
      {loading && <p className="list-placeholder">Loading channel...</p>}
      {error && <p className="form-error">{error}</p>}
      {channel && (
        <>
          <div className="channel-detail-header">
            <div className="channel-detail-header-left">
              <span className="channel-type-badge-lg">
                {channel.channel_type}
              </span>
              <div>
                <h3 className="channel-detail-name">{channel.name}</h3>
                <span className="channel-detail-created">
                  Created {formatDate(channel.created_at)}
                </span>
              </div>
              <span
                className="channel-status-badge"
                style={{ background: CHANNEL_STATUS_COLORS[channel.status] }}
              >
                {channel.status}
              </span>
            </div>
            <div className="agent-detail-actions">
              <button
                className={
                  channel.status === "connected"
                    ? "btn-channel-disconnect"
                    : "btn-channel-connect"
                }
                onClick={handleToggleConnection}
                disabled={toggling}
                aria-label={
                  channel.status === "connected"
                    ? "Disconnect channel"
                    : "Connect channel"
                }
              >
                {toggling
                  ? "..."
                  : channel.status === "connected"
                    ? "Disconnect"
                    : "Connect"}
              </button>
              <button
                className="btn-icon btn-danger"
                onClick={handleDelete}
                title="Delete channel"
                aria-label="Delete channel"
              >
                <Trash2 size={16} />
              </button>
            </div>
          </div>

          <div className="channel-config-section">
            <h3 className="channel-section-title">Configuration</h3>

            {mutationError && <p className="form-error">{mutationError}</p>}

            <div className="channel-form">
              <label className="form-label">
                Channel Type
                <input
                  type="text"
                  className="form-input"
                  value={CHANNEL_TYPE_DISPLAY[channel.channel_type]}
                  readOnly
                  aria-label="Channel type"
                />
              </label>

              <label className="form-label">
                Name
                <input
                  type="text"
                  className="form-input"
                  value={editName}
                  onChange={(e) => setEditName(e.target.value)}
                  aria-label="Channel name"
                />
              </label>

              <label className="form-label">
                Bound Agent
                <select
                  className="form-input"
                  value={editAgentId}
                  onChange={(e) => setEditAgentId(e.target.value)}
                  aria-label="Select agent"
                >
                  <option value="">Select agent...</option>
                  {agents.map((a) => (
                    <option key={a.id} value={a.id}>
                      {a.name}
                    </option>
                  ))}
                </select>
              </label>

              {channel.channel_type === "telegram" && (
                <label className="form-label">
                  Bot Token
                  <input
                    type="text"
                    className="form-input"
                    value={editConfig.bot_token ?? ""}
                    onChange={(e) =>
                      setEditConfig((c) => ({
                        ...c,
                        bot_token: e.target.value,
                      }))
                    }
                    placeholder="123456:ABC-DEF..."
                    aria-label="Telegram bot token"
                  />
                </label>
              )}

              {channel.channel_type === "lark" && (
                <>
                  <label className="form-label">
                    App ID
                    <input
                      type="text"
                      className="form-input"
                      value={editConfig.app_id ?? ""}
                      onChange={(e) =>
                        setEditConfig((c) => ({
                          ...c,
                          app_id: e.target.value,
                        }))
                      }
                      placeholder="cli_xxxxx"
                      aria-label="Lark app ID"
                    />
                  </label>
                  <label className="form-label">
                    App Secret
                    <input
                      type="text"
                      className="form-input"
                      value={editConfig.app_secret ?? ""}
                      onChange={(e) =>
                        setEditConfig((c) => ({
                          ...c,
                          app_secret: e.target.value,
                        }))
                      }
                      placeholder="App secret"
                      aria-label="Lark app secret"
                    />
                  </label>
                </>
              )}

              {channel.channel_type !== "telegram" &&
                channel.channel_type !== "lark" && (
                  <label className="form-label">
                    Configuration (JSON)
                    <textarea
                      className="form-textarea"
                      value={editConfigJson}
                      onChange={(e) => setEditConfigJson(e.target.value)}
                      rows={6}
                      aria-label="Channel configuration JSON"
                    />
                  </label>
                )}

              <div className="form-actions">
                <button
                  className="btn-primary"
                  onClick={handleSave}
                  disabled={saving}
                  aria-label="Save channel configuration"
                >
                  {saving ? "Saving..." : "Save"}
                </button>
              </div>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
