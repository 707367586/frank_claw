import { useState, useEffect, useCallback } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Trash2,
  Pause,
  Play,
  Archive,
  Plus,
  ChevronDown,
  ChevronRight,
  Check,
  X,
  BellOff,
  TrendingDown,
  Save,
} from "lucide-react";
import {
  getTask,
  updateTask,
  deleteTask,
  pauseTask,
  resumeTask,
  archiveTask,
  listTriggers,
  addTrigger,
  deleteTrigger,
  listRuns,
  submitFeedback,
  listAgents,
} from "../lib/api";
import type { Agent, Task, Trigger, Run } from "../lib/types";

const LIFECYCLE_COLORS: Record<Task["lifecycle_status"], string> = {
  active: "#4ade80",
  paused: "#facc15",
  archived: "#6b7280",
};

const TRIGGER_KIND_COLORS: Record<Trigger["kind"], string> = {
  time: "#60a5fa",
  event: "#a78bfa",
  context: "#34d399",
  policy: "#fbbf24",
};

const RUN_STATUS_COLORS: Record<Run["status"], string> = {
  queued: "#6b7280",
  planning: "#a78bfa",
  running: "#60a5fa",
  waiting_confirmation: "#facc15",
  completed: "#4ade80",
  failed: "#f87171",
  interrupted: "#fb923c",
};

function formatDate(dateStr: string | null): string {
  if (!dateStr) return "-";
  return new Date(dateStr).toLocaleDateString("en-US", {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function formatDuration(start: string | null, end: string | null): string {
  if (!start || !end) return "-";
  const ms = new Date(end).getTime() - new Date(start).getTime();
  if (ms < 1000) return `${ms}ms`;
  if (ms < 60_000) return `${(ms / 1000).toFixed(1)}s`;
  return `${Math.round(ms / 60_000)}m`;
}

type Tab = "triggers" | "runs" | "edit";

export default function TasksPage() {
  const [searchParams, setSearchParams] = useSearchParams();
  const taskId = searchParams.get("task");

  const [task, setTask] = useState<Task | null>(null);
  const [agentName, setAgentName] = useState<string>("");
  const [triggers, setTriggers] = useState<Trigger[]>([]);
  const [runs, setRuns] = useState<Run[]>([]);
  const [activeTab, setActiveTab] = useState<Tab>("triggers");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);

  // Trigger add form
  const [showTriggerForm, setShowTriggerForm] = useState(false);
  const [triggerKind, setTriggerKind] = useState<Trigger["kind"]>("time");
  const [triggerConfig, setTriggerConfig] = useState("");
  const [addingTrigger, setAddingTrigger] = useState(false);

  // Run expansion
  const [expandedRunId, setExpandedRunId] = useState<string | null>(null);

  // Edit form
  const [editName, setEditName] = useState("");
  const [editGoal, setEditGoal] = useState("");
  const [editMaxSteps, setEditMaxSteps] = useState(10);
  const [editTimeout, setEditTimeout] = useState(300);
  const [saving, setSaving] = useState(false);

  const loadTask = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const [taskData, agentData] = await Promise.all([
        getTask(id),
        listAgents(),
      ]);
      setTask(taskData);
      const agent = agentData.find((a: Agent) => a.id === taskData.agent_id);
      setAgentName(agent?.name ?? "Unknown agent");
      setEditName(taskData.name);
      setEditGoal(taskData.goal);
      setEditMaxSteps(taskData.default_max_steps);
      setEditTimeout(taskData.default_timeout_secs);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load task");
    } finally {
      setLoading(false);
    }
  }, []);

  const loadTriggers = useCallback(async (id: string) => {
    try {
      const data = await listTriggers(id);
      setTriggers(data);
    } catch {
      // non-blocking
    }
  }, []);

  const loadRuns = useCallback(async (id: string) => {
    try {
      const data = await listRuns(id);
      setRuns(data);
    } catch {
      // non-blocking
    }
  }, []);

  useEffect(() => {
    if (!taskId) {
      setTask(null);
      setTriggers([]);
      setRuns([]);
      return;
    }
    loadTask(taskId);
    loadTriggers(taskId);
    loadRuns(taskId);
  }, [taskId, loadTask, loadTriggers, loadRuns]);

  const handlePause = useCallback(async () => {
    if (!task) return;
    setMutationError(null);
    try {
      const updated = await pauseTask(task.id);
      setTask(updated);
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to pause task",
      );
    }
  }, [task]);

  const handleResume = useCallback(async () => {
    if (!task) return;
    setMutationError(null);
    try {
      const updated = await resumeTask(task.id);
      setTask(updated);
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to resume task",
      );
    }
  }, [task]);

  const handleArchive = useCallback(async () => {
    if (!task) return;
    const confirmed = window.confirm(
      `Archive task "${task.name}"? It will no longer run.`,
    );
    if (!confirmed) return;
    setMutationError(null);
    try {
      const updated = await archiveTask(task.id);
      setTask(updated);
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to archive task",
      );
    }
  }, [task]);

  const handleDelete = useCallback(async () => {
    if (!task) return;
    const confirmed = window.confirm(
      `Delete task "${task.name}"? This action cannot be undone.`,
    );
    if (!confirmed) return;
    try {
      await deleteTask(task.id);
      setTask(null);
      setTriggers([]);
      setRuns([]);
      setSearchParams({});
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete task");
    }
  }, [task, setSearchParams]);

  const handleAddTrigger = useCallback(async () => {
    if (!task || !triggerConfig.trim()) return;
    setAddingTrigger(true);
    setMutationError(null);
    try {
      const config: Record<string, unknown> =
        triggerKind === "time"
          ? { cron: triggerConfig.trim() }
          : { event_type: triggerConfig.trim() };
      const created = await addTrigger(task.id, {
        kind: triggerKind,
        config,
        status: "active",
      });
      setTriggers((prev) => [created, ...prev]);
      setTriggerConfig("");
      setShowTriggerForm(false);
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to add trigger",
      );
    } finally {
      setAddingTrigger(false);
    }
  }, [task, triggerKind, triggerConfig]);

  const handleDeleteTrigger = useCallback(async (triggerId: string) => {
    setMutationError(null);
    try {
      await deleteTrigger(triggerId);
      setTriggers((prev) => prev.filter((t) => t.id !== triggerId));
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to delete trigger",
      );
    }
  }, []);

  const handleFeedback = useCallback(
    async (runId: string, kind: string) => {
      setMutationError(null);
      try {
        await submitFeedback(runId, kind);
        setRuns((prev) =>
          prev.map((r) =>
            r.id === runId ? { ...r, feedback_kind: kind } : r,
          ),
        );
      } catch (err) {
        setMutationError(
          err instanceof Error ? err.message : "Failed to submit feedback",
        );
      }
    },
    [],
  );

  const handleSaveEdit = useCallback(async () => {
    if (!task) return;
    setSaving(true);
    setMutationError(null);
    try {
      const updated = await updateTask(task.id, {
        name: editName.trim(),
        goal: editGoal.trim(),
        default_max_steps: editMaxSteps,
        default_timeout_secs: editTimeout,
      });
      setTask(updated);
    } catch (err) {
      setMutationError(
        err instanceof Error ? err.message : "Failed to update task",
      );
    } finally {
      setSaving(false);
    }
  }, [task, editName, editGoal, editMaxSteps, editTimeout]);

  // Empty state
  if (!taskId) {
    return (
      <div className="empty-state">
        <h2>Scheduled Tasks</h2>
        <p>Select a task from the sidebar to view details.</p>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="empty-state">
        <p>Loading task...</p>
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

  if (!task) {
    return (
      <div className="empty-state">
        <h2>Task not found</h2>
        <p>The selected task could not be loaded.</p>
      </div>
    );
  }

  return (
    <div className="agent-detail">
      {/* Header */}
      <div className="agent-detail-header">
        <div className="agent-detail-header-left">
          <div>
            <h2 className="agent-detail-name">{task.name}</h2>
            <span className="agent-detail-role">
              {agentName} &middot; {task.goal}
            </span>
          </div>
          <span
            className="agent-status-badge"
            style={{
              background: LIFECYCLE_COLORS[task.lifecycle_status],
              color: task.lifecycle_status === "paused" ? "#1a1a2e" : "#fff",
            }}
          >
            {task.lifecycle_status}
          </span>
        </div>
        <div className="agent-detail-actions">
          {task.lifecycle_status === "active" && (
            <button
              className="btn-icon"
              onClick={handlePause}
              title="Pause task"
              aria-label="Pause task"
            >
              <Pause size={16} />
            </button>
          )}
          {task.lifecycle_status === "paused" && (
            <button
              className="btn-icon"
              onClick={handleResume}
              title="Resume task"
              aria-label="Resume task"
            >
              <Play size={16} />
            </button>
          )}
          {task.lifecycle_status !== "archived" && (
            <button
              className="btn-icon"
              onClick={handleArchive}
              title="Archive task"
              aria-label="Archive task"
            >
              <Archive size={16} />
            </button>
          )}
          <button
            className="btn-icon btn-danger"
            onClick={handleDelete}
            title="Delete task"
            aria-label="Delete task"
          >
            <Trash2 size={16} />
          </button>
        </div>
      </div>

      {mutationError && (
        <div style={{ padding: "0 24px" }}>
          <p className="form-error">{mutationError}</p>
        </div>
      )}

      {/* Tabs */}
      <div className="tabs">
        {(["triggers", "runs", "edit"] as Tab[]).map((tab) => (
          <button
            key={tab}
            className={`tab ${activeTab === tab ? "active" : ""}`}
            onClick={() => setActiveTab(tab)}
            aria-label={`${tab} tab`}
          >
            {tab === "triggers"
              ? "Triggers"
              : tab === "runs"
                ? "Run History"
                : "Edit"}
          </button>
        ))}
      </div>

      {/* Tab content */}
      <div className="tab-content">
        {activeTab === "triggers" && (
          <div className="task-triggers-section">
            <div className="task-section-header">
              <h3 className="task-section-title">Triggers</h3>
              <button
                className="btn-primary"
                onClick={() => setShowTriggerForm((v) => !v)}
                aria-label="Add trigger"
              >
                <Plus size={14} /> Add Trigger
              </button>
            </div>

            {showTriggerForm && (
              <div className="task-inline-form">
                <select
                  className="form-input"
                  aria-label="Trigger kind"
                  value={triggerKind}
                  onChange={(e) =>
                    setTriggerKind(e.target.value as Trigger["kind"])
                  }
                >
                  <option value="time">Time (Cron)</option>
                  <option value="event">Event</option>
                  <option value="context">Context</option>
                  <option value="policy">Policy</option>
                </select>
                <input
                  type="text"
                  className="form-input"
                  placeholder={
                    triggerKind === "time"
                      ? "Cron expression (e.g. 0 9 * * *)"
                      : "Event type or expression"
                  }
                  aria-label="Trigger configuration"
                  value={triggerConfig}
                  onChange={(e) => setTriggerConfig(e.target.value)}
                />
                <div className="form-actions">
                  <button
                    className="btn-secondary"
                    onClick={() => setShowTriggerForm(false)}
                    disabled={addingTrigger}
                  >
                    Cancel
                  </button>
                  <button
                    className="btn-primary"
                    onClick={handleAddTrigger}
                    disabled={addingTrigger || !triggerConfig.trim()}
                    aria-label="Confirm add trigger"
                  >
                    {addingTrigger ? "Adding..." : "Add"}
                  </button>
                </div>
              </div>
            )}

            {triggers.length === 0 ? (
              <p className="list-placeholder">No triggers configured</p>
            ) : (
              <div className="task-trigger-list">
                {triggers.map((trigger) => (
                  <div key={trigger.id} className="task-trigger-item">
                    <div className="task-trigger-item-top">
                      <span
                        className="trigger-kind-badge"
                        style={{
                          background: TRIGGER_KIND_COLORS[trigger.kind],
                        }}
                      >
                        {trigger.kind}
                      </span>
                      <span className="task-trigger-config">
                        {trigger.kind === "time"
                          ? (trigger.config.cron as string) ?? "N/A"
                          : (trigger.config.event_type as string) ?? "N/A"}
                      </span>
                      <span
                        className="task-trigger-status"
                        style={{
                          color:
                            trigger.status === "active"
                              ? "#4ade80"
                              : "#facc15",
                        }}
                      >
                        {trigger.status}
                      </span>
                      <button
                        className="btn-icon-sm btn-danger"
                        onClick={() => handleDeleteTrigger(trigger.id)}
                        title="Delete trigger"
                        aria-label="Delete trigger"
                      >
                        <Trash2 size={14} />
                      </button>
                    </div>
                    <div className="task-trigger-meta">
                      <span>Next: {formatDate(trigger.next_fire_at)}</span>
                      <span>Last: {formatDate(trigger.last_fired_at)}</span>
                    </div>
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "runs" && (
          <div className="task-runs-section">
            <h3 className="task-section-title">Run History</h3>
            {runs.length === 0 ? (
              <p className="list-placeholder">No runs yet</p>
            ) : (
              <div className="task-run-list">
                {runs.map((run) => (
                  <div key={run.id} className="task-run-item">
                    <button
                      className="task-run-item-header"
                      onClick={() =>
                        setExpandedRunId(
                          expandedRunId === run.id ? null : run.id,
                        )
                      }
                      aria-label={`Toggle run details ${run.id}`}
                    >
                      {expandedRunId === run.id ? (
                        <ChevronDown size={14} />
                      ) : (
                        <ChevronRight size={14} />
                      )}
                      <span
                        className="run-status-badge"
                        style={{ background: RUN_STATUS_COLORS[run.status] }}
                      >
                        {run.status}
                      </span>
                      <span className="task-run-time">
                        {formatDate(run.started_at)}
                      </span>
                      <span className="task-run-duration">
                        {formatDuration(run.started_at, run.completed_at)}
                      </span>
                      {run.feedback_kind && (
                        <span className="task-run-feedback-indicator">
                          {run.feedback_kind}
                        </span>
                      )}
                    </button>

                    {expandedRunId === run.id && (
                      <div className="task-run-detail">
                        {run.checkpoint &&
                          Object.keys(run.checkpoint).length > 0 && (
                            <div className="task-run-checkpoint">
                              <span className="profile-field-label">
                                Checkpoint
                              </span>
                              <pre className="task-run-checkpoint-pre">
                                {JSON.stringify(run.checkpoint, null, 2)}
                              </pre>
                            </div>
                          )}
                        <div className="task-run-feedback-actions">
                          <span className="profile-field-label">Feedback</span>
                          <div className="task-feedback-btns">
                            <button
                              className={`task-feedback-btn ${run.feedback_kind === "accepted" ? "active" : ""}`}
                              onClick={() =>
                                handleFeedback(run.id, "accepted")
                              }
                              aria-label="Accept run"
                              title="Accepted"
                            >
                              <Check size={14} /> Accept
                            </button>
                            <button
                              className={`task-feedback-btn ${run.feedback_kind === "rejected" ? "active" : ""}`}
                              onClick={() =>
                                handleFeedback(run.id, "rejected")
                              }
                              aria-label="Reject run"
                              title="Rejected"
                            >
                              <X size={14} /> Reject
                            </button>
                            <button
                              className={`task-feedback-btn ${run.feedback_kind === "mute_forever" ? "active" : ""}`}
                              onClick={() =>
                                handleFeedback(run.id, "mute_forever")
                              }
                              aria-label="Mute forever"
                              title="Mute forever"
                            >
                              <BellOff size={14} /> Mute
                            </button>
                            <button
                              className={`task-feedback-btn ${run.feedback_kind === "reduce_frequency" ? "active" : ""}`}
                              onClick={() =>
                                handleFeedback(run.id, "reduce_frequency")
                              }
                              aria-label="Reduce frequency"
                              title="Reduce frequency"
                            >
                              <TrendingDown size={14} /> Less
                            </button>
                          </div>
                        </div>
                      </div>
                    )}
                  </div>
                ))}
              </div>
            )}
          </div>
        )}

        {activeTab === "edit" && (
          <div className="profile-section">
            <label className="form-label">
              Name
              <input
                type="text"
                className="form-input"
                value={editName}
                onChange={(e) => setEditName(e.target.value)}
                aria-label="Task name"
              />
            </label>
            <label className="form-label">
              Goal
              <textarea
                className="form-textarea"
                value={editGoal}
                onChange={(e) => setEditGoal(e.target.value)}
                aria-label="Task goal"
                rows={3}
              />
            </label>
            <label className="form-label">
              Max Steps
              <input
                type="number"
                className="form-input"
                value={editMaxSteps}
                onChange={(e) => setEditMaxSteps(Number(e.target.value))}
                aria-label="Max steps"
                min={1}
              />
            </label>
            <label className="form-label">
              Timeout (seconds)
              <input
                type="number"
                className="form-input"
                value={editTimeout}
                onChange={(e) => setEditTimeout(Number(e.target.value))}
                aria-label="Timeout seconds"
                min={1}
              />
            </label>
            <div className="form-actions">
              <button
                className="btn-primary"
                onClick={handleSaveEdit}
                disabled={saving}
                aria-label="Save task changes"
              >
                <Save size={14} /> {saving ? "Saving..." : "Save Changes"}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
