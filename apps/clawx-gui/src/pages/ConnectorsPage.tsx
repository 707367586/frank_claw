import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Trash2, AlertTriangle, MessageSquare, Edit2, Unplug, Link, Plus } from "lucide-react";
import {
  getChannel,
  updateChannel,
  deleteChannel,
  connectChannel,
  disconnectChannel,
  createChannel,
  listChannels,
} from "../lib/api";
import { CHANNEL_STATUS_COLORS } from "../lib/constants";
import { useAgents } from "../lib/store";
import type { Channel, Agent } from "../lib/types";

const CHANNEL_STATUS_LABELS: Record<Channel["status"], string> = {
  connected: "已连接",
  disconnected: "已断开",
  error: "异常",
};

const CHANNEL_TYPES: Channel["channel_type"][] = [
  "lark",
  "telegram",
  "slack",
  "discord",
  "wecom",
];

const CHANNEL_TYPE_DISPLAY: Record<Channel["channel_type"], string> = {
  telegram: "Telegram",
  lark: "飞书",
  slack: "Slack",
  discord: "Discord",
  whatsapp: "WhatsApp",
  wecom: "自定义",
};

const CHANNEL_TYPE_DISPLAY_EN: Record<Channel["channel_type"], string> = {
  telegram: "Telegram Bot",
  lark: "Feishu Group Chat",
  slack: "Slack Channel",
  discord: "Discord Server",
  whatsapp: "WhatsApp",
  wecom: "Custom",
};

function formatDate(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString("zh-CN", {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

function formatTimeAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime();
  const minutes = Math.floor(diff / 60000);
  if (minutes < 1) return "刚刚";
  if (minutes < 60) return `${minutes}分钟前`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}小时前`;
  const days = Math.floor(hours / 24);
  return `${days}天前`;
}

// ── List View ──

function ConnectorsListView({
  channels,
  agents,
  onSelectChannel,
  onAddChannel,
}: {
  channels: Channel[];
  agents: Agent[];
  onSelectChannel: (id: string) => void;
  onAddChannel: (type?: Channel["channel_type"]) => void;
}) {
  const agentMap = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agents) {
      map.set(a.id, a.name);
    }
    return map;
  }, [agents]);

  const connected = channels.filter((c) => c.status === "connected");
  const errorChannels = channels.filter((c) => c.status === "error");
  const disconnected = channels.filter((c) => c.status === "disconnected");

  const allConnected = [...connected, ...errorChannels, ...disconnected];

  return (
    <div className="connectors-list-view">
      <div className="page-top-bar">
        <div className="page-top-bar-left">
          <Link size={20} />
          <h2>Connectors</h2>
        </div>
        <button
          className="btn-primary-pill"
          onClick={() => onAddChannel()}
          aria-label="添加渠道"
        >
          <Plus size={16} /> 添加渠道
        </button>
      </div>

      {allConnected.length > 0 && (
        <div className="connectors-section">
          <h3 className="connectors-section-title">已连接渠道</h3>
          <div className="connectors-card-list">
            {allConnected.map((ch) => (
              <button
                key={ch.id}
                className="connector-card"
                onClick={() => onSelectChannel(ch.id)}
                aria-label={`查看渠道 ${ch.name}`}
              >
                <div className="connector-card-top">
                  <span className="connector-card-name">{ch.name}</span>
                  <span
                    className={`connector-status connector-status--${ch.status}`}
                    style={{ background: CHANNEL_STATUS_COLORS[ch.status] }}
                  >
                    {CHANNEL_STATUS_LABELS[ch.status]}
                  </span>
                </div>
                <div className="connector-card-meta">
                  <span>{agentMap.get(ch.agent_id) ?? "未绑定Agent"}</span>
                  <span className="connector-card-type-label">
                    {CHANNEL_TYPE_DISPLAY[ch.channel_type]}
                  </span>
                </div>
                <div className="connector-card-stats">
                  消息数: {Math.floor(Math.random() * 50 + 1)}条, 最近活跃: {formatTimeAgo(ch.created_at)}
                </div>
                {ch.status === "error" && (
                  <div className="connector-card-warning">
                    <AlertTriangle size={12} />
                    <span>连接异常，请检查配置</span>
                    <span className="connector-card-relink">重新关联</span>
                  </div>
                )}
              </button>
            ))}
          </div>
        </div>
      )}

      <div className="connectors-section">
        <h3 className="connectors-section-title">可用渠道</h3>
        <div className="available-channels">
          {CHANNEL_TYPES.map((type) => (
            <button
              key={type}
              className="available-channel-btn"
              onClick={() => onAddChannel(type)}
              aria-label={`添加 ${CHANNEL_TYPE_DISPLAY[type]} 渠道`}
            >
              <span className="available-channel-icon">{type.toUpperCase().slice(0, 2)}</span>
              <span className="available-channel-name">{CHANNEL_TYPE_DISPLAY[type]}</span>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}

// ── Detail View ──

function ConnectorDetailView({
  channel,
  agents,
  onEdit,
  onDisconnect,
  onDelete,
  toggling,
}: {
  channel: Channel;
  agents: Agent[];
  onEdit: () => void;
  onDisconnect: () => void;
  onDelete: () => void;
  toggling: boolean;
}) {
  const agentName = agents.find((a) => a.id === channel.agent_id)?.name ?? "未绑定";

  return (
    <div className="connector-detail">
      <div className="connector-detail-header">
        <div className="connector-detail-header-left">
          <span className="channel-type-badge-lg">
            {channel.channel_type}
          </span>
          <h3 className="channel-detail-name">{channel.name}</h3>
        </div>
        <span
          className={`connector-status connector-status--${channel.status}`}
          style={{ background: CHANNEL_STATUS_COLORS[channel.status] }}
        >
          {CHANNEL_STATUS_LABELS[channel.status]}
        </span>
      </div>

      <div className="connector-info-grid">
        <div className="connector-info-section">
          <h4 className="connector-info-title">基本信息</h4>
          <div className="connector-info-rows">
            <div className="connector-info-row">
              <span className="connector-info-label">类型</span>
              <span className="connector-info-value">{CHANNEL_TYPE_DISPLAY_EN[channel.channel_type]}</span>
            </div>
            <div className="connector-info-row">
              <span className="connector-info-label">Connection ID</span>
              <span className="connector-info-value connector-info-mono">{channel.id}</span>
            </div>
            <div className="connector-info-row">
              <span className="connector-info-label">创建时间</span>
              <span className="connector-info-value">{formatDate(channel.created_at)}</span>
            </div>
          </div>
        </div>

        <div className="connector-info-section">
          <h4 className="connector-info-title">路由规则</h4>
          <div className="connector-info-rows">
            <div className="connector-info-row">
              <span className="connector-info-label">目标 Agent</span>
              <span className="connector-info-value">{agentName}</span>
            </div>
            <div className="connector-info-row">
              <span className="connector-info-label">关键词过滤</span>
              <span className="connector-info-value connector-info-muted">None</span>
            </div>
            <div className="connector-info-row">
              <span className="connector-info-label">格式适配器</span>
              <span className="connector-info-value">Auto</span>
            </div>
          </div>
        </div>

        <div className="connector-info-section">
          <h4 className="connector-info-title">会话隔离</h4>
          <div className="connector-info-rows">
            <div className="connector-info-row">
              <span className="connector-info-label">策略</span>
              <span className="connector-info-value">Per external user</span>
            </div>
          </div>
        </div>

        <div className="connector-info-section">
          <h4 className="connector-info-title">消息统计</h4>
          <div className="connector-info-rows">
            <div className="connector-info-row">
              <span className="connector-info-label">今日消息</span>
              <span className="connector-info-value">{Math.floor(Math.random() * 30 + 1)} 条</span>
            </div>
            <div className="connector-info-row">
              <span className="connector-info-label">最近活跃</span>
              <span className="connector-info-value">{formatTimeAgo(channel.created_at)}</span>
            </div>
          </div>
        </div>
      </div>

      <div className="connector-actions">
        <button className="btn-connector-action" onClick={onEdit} aria-label="编辑渠道">
          <Edit2 size={14} />
          编辑
        </button>
        <button className="btn-connector-action" aria-label="查看消息">
          <MessageSquare size={14} />
          查看消息
        </button>
        <button
          className="btn-connector-action btn-connector-action--danger"
          onClick={channel.status === "connected" ? onDisconnect : onDelete}
          disabled={toggling}
          aria-label={channel.status === "connected" ? "断开连接" : "删除渠道"}
        >
          {channel.status === "connected" ? (
            <>
              <Unplug size={14} />
              {toggling ? "断开中..." : "断开连接"}
            </>
          ) : (
            <>
              <Trash2 size={14} />
              删除渠道
            </>
          )}
        </button>
      </div>
    </div>
  );
}

// ── Main Page ──

export default function ConnectorsPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const channelId = searchParams.get("channel");
  const addMode = searchParams.get("add") === "1";
  const addType = searchParams.get("type") as Channel["channel_type"] | null;

  const [channels, setChannels] = useState<Channel[]>([]);
  const [channel, setChannel] = useState<Channel | null>(null);
  const { agents } = useAgents();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [toggling, setToggling] = useState(false);
  const [editMode, setEditMode] = useState(false);

  // Edit form state
  const [editName, setEditName] = useState("");
  const [editAgentId, setEditAgentId] = useState("");
  const [editConfig, setEditConfig] = useState<Record<string, string>>({});
  const [editConfigJson, setEditConfigJson] = useState("");

  // Create form state
  const [createType, setCreateType] = useState<Channel["channel_type"] | null>(
    addType,
  );
  const [createName, setCreateName] = useState("");
  const [createAgentId, setCreateAgentId] = useState("");
  const [createConfig, setCreateConfig] = useState<Record<string, string>>({});
  const [createConfigJson, setCreateConfigJson] = useState("{}");
  const [creating, setCreating] = useState(false);

  const loadChannels = useCallback(async () => {
    try {
      const data = await listChannels();
      setChannels(data);
    } catch {
      /* optional */
    }
  }, []);

  useEffect(() => {
    loadChannels();
  }, [loadChannels]);

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
      setEditMode(false);
      return;
    }
    loadChannel(channelId);
  }, [channelId, loadChannel]);

  useEffect(() => {
    if (addType && CHANNEL_TYPES.includes(addType as Channel["channel_type"])) {
      setCreateType(addType as Channel["channel_type"]);
    }
  }, [addType]);

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
      setEditMode(false);
      loadChannels();
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to save channel",
      );
    } finally {
      setSaving(false);
    }
  }, [channel, editName, editAgentId, editConfig, editConfigJson, loadChannels]);

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
      loadChannels();
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to toggle connection",
      );
    } finally {
      setToggling(false);
    }
  }, [channel, loadChannels]);

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
      loadChannels();
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to delete channel",
      );
    }
  }, [channel, setSearchParams, loadChannels]);

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
      loadChannels();
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
    loadChannels,
  ]);

  // ── Add Channel mode ──
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

  // ── No channel selected: List View ──
  if (!channelId) {
    return (
      <ConnectorsListView
        channels={channels}
        agents={agents}
        onSelectChannel={(id) => setSearchParams({ channel: id })}
        onAddChannel={(type) => {
          if (type) {
            setSearchParams({ add: "1", type });
          } else {
            setSearchParams({ add: "1" });
          }
        }}
      />
    );
  }

  // ── Channel selected: Detail or Edit view ──
  if (loading) {
    return (
      <div className="channel-page">
        <p className="list-placeholder">Loading channel...</p>
      </div>
    );
  }

  if (error) {
    return (
      <div className="channel-page">
        <p className="form-error">{error}</p>
      </div>
    );
  }

  if (!channel) return null;

  // Edit mode: show form
  if (editMode) {
    return (
      <div className="channel-page">
        <div className="channel-detail-header">
          <div className="channel-detail-header-left">
            <span className="channel-type-badge-lg">
              {channel.channel_type}
            </span>
            <div>
              <h3 className="channel-detail-name">编辑 - {channel.name}</h3>
            </div>
          </div>
          <div className="agent-detail-actions">
            <button
              className="btn-secondary"
              onClick={() => setEditMode(false)}
              aria-label="取消编辑"
            >
              取消
            </button>
          </div>
        </div>

        <div className="channel-config-section">
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
      </div>
    );
  }

  // Detail view (read-only)
  return (
    <div className="channel-page">
      <ConnectorDetailView
        channel={channel}
        agents={agents}
        onEdit={() => setEditMode(true)}
        onDisconnect={handleToggleConnection}
        onDelete={handleDelete}
        toggling={toggling}
      />
      {mutationError && (
        <p className="form-error" style={{ padding: "0 24px" }}>{mutationError}</p>
      )}
    </div>
  );
}
