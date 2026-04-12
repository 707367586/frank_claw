import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import { Search, Plus, Clock, CalendarClock } from "lucide-react";
import { listTasks, listAgents } from "../lib/api";
import type { Agent, Task } from "../lib/types";

const LIFECYCLE_COLORS: Record<Task["lifecycle_status"], string> = {
  active: "#4ade80",
  paused: "#facc15",
  archived: "#6b7280",
};

function formatNextFire(dateStr: string | null | undefined): string {
  if (!dateStr) return "No schedule";
  const d = new Date(dateStr);
  const now = new Date();
  const diff = d.getTime() - now.getTime();
  if (diff < 0) return "Overdue";
  if (diff < 3600_000) return `${Math.round(diff / 60_000)}m`;
  if (diff < 86400_000) return `${Math.round(diff / 3600_000)}h`;
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export default function TaskList() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selectedId = searchParams.get("task");

  const [tasks, setTasks] = useState<Task[]>([]);
  const [agents, setAgents] = useState<Map<string, string>>(new Map());
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const [showCreateForm, setShowCreateForm] = useState(false);
  const [newName, setNewName] = useState("");
  const [newGoal, setNewGoal] = useState("");
  const [newAgentId, setNewAgentId] = useState("");
  const [createError, setCreateError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [agentList, setAgentList] = useState<Agent[]>([]);

  const loadData = useCallback(async () => {
    try {
      setError(null);
      const [taskData, agentData] = await Promise.all([
        listTasks(),
        listAgents(),
      ]);
      setTasks(taskData);
      setAgentList(agentData);
      const map = new Map<string, string>();
      for (const a of agentData) {
        map.set(a.id, a.name);
      }
      setAgents(map);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to load tasks");
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadData();
  }, [loadData]);

  const filtered = useMemo(() => {
    const q = search.toLowerCase();
    if (!q) return tasks;
    return tasks.filter(
      (t) =>
        t.name.toLowerCase().includes(q) ||
        t.goal.toLowerCase().includes(q) ||
        (agents.get(t.agent_id) ?? "").toLowerCase().includes(q),
    );
  }, [tasks, search, agents]);

  const handleSelect = useCallback(
    (id: string) => {
      setSearchParams({ task: id });
    },
    [setSearchParams],
  );

  const handleCreate = useCallback(async () => {
    if (!newName.trim() || !newAgentId.trim()) {
      setCreateError("Task name and agent are required.");
      return;
    }
    setCreating(true);
    setCreateError(null);
    try {
      const { createTask } = await import("../lib/api");
      const created = await createTask({
        name: newName.trim(),
        goal: newGoal.trim(),
        agent_id: newAgentId.trim(),
        source_kind: "manual",
        lifecycle_status: "active",
        notification_policy: "on_failure",
        default_max_steps: 10,
        default_timeout_secs: 300,
      });
      setTasks((prev) => [created, ...prev]);
      setNewName("");
      setNewGoal("");
      setNewAgentId("");
      setShowCreateForm(false);
      setSearchParams({ task: created.id });
    } catch (err) {
      setCreateError(
        err instanceof Error ? err.message : "Failed to create task",
      );
    } finally {
      setCreating(false);
    }
  }, [newName, newGoal, newAgentId, setSearchParams]);

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <div className="list-panel-header-row">
          <h2 className="list-panel-title">Scheduled Tasks</h2>
          <button
            className="new-chat-btn"
            onClick={() => setShowCreateForm((v) => !v)}
            title="Create Task"
            aria-label="Create task"
          >
            <Plus size={16} />
          </button>
        </div>
      </div>

      {showCreateForm && (
        <div className="kb-add-form">
          <input
            type="text"
            className="form-input"
            placeholder="Task name"
            aria-label="Task name"
            value={newName}
            onChange={(e) => setNewName(e.target.value)}
          />
          <input
            type="text"
            className="form-input"
            placeholder="Goal (what should it accomplish?)"
            aria-label="Task goal"
            value={newGoal}
            onChange={(e) => setNewGoal(e.target.value)}
          />
          <select
            className="form-input"
            aria-label="Agent for task"
            value={newAgentId}
            onChange={(e) => setNewAgentId(e.target.value)}
          >
            <option value="">Select agent...</option>
            {agentList.map((a) => (
              <option key={a.id} value={a.id}>
                {a.name}
              </option>
            ))}
          </select>
          {createError && <p className="form-error">{createError}</p>}
          <div className="form-actions">
            <button
              className="btn-secondary"
              onClick={() => {
                setShowCreateForm(false);
                setCreateError(null);
              }}
              disabled={creating}
            >
              Cancel
            </button>
            <button
              className="btn-primary"
              onClick={handleCreate}
              disabled={creating}
              aria-label="Confirm create task"
            >
              {creating ? "Creating..." : "Create"}
            </button>
          </div>
        </div>
      )}

      <div className="list-panel-search">
        <Search size={14} className="search-icon" />
        <input
          type="text"
          className="search-input"
          aria-label="Search tasks"
          placeholder="Search tasks..."
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </div>

      <div className="list-panel-content">
        {loading && <p className="list-placeholder">Loading...</p>}
        {error && <p className="list-placeholder">{error}</p>}
        {!loading && !error && filtered.length === 0 && (
          <p className="list-placeholder">
            {search ? "No matches" : "No tasks yet"}
          </p>
        )}
        {filtered.map((task) => (
          <button
            key={task.id}
            className={`task-card ${selectedId === task.id ? "selected" : ""}`}
            onClick={() => handleSelect(task.id)}
            aria-label={`Select task ${task.name}`}
          >
            <div className="task-card-icon">
              <CalendarClock size={18} />
            </div>
            <div className="task-card-info">
              <span className="task-card-name">{task.name}</span>
              <span className="task-card-agent">
                {agents.get(task.agent_id) ?? "Unknown agent"}
              </span>
            </div>
            <div className="task-card-right">
              <span
                className="task-status-badge"
                style={{ background: LIFECYCLE_COLORS[task.lifecycle_status] }}
                title={task.lifecycle_status}
              >
                {task.lifecycle_status}
              </span>
              <span className="task-card-next">
                <Clock size={10} />
                {formatNextFire(null)}
              </span>
            </div>
          </button>
        ))}
      </div>
    </aside>
  );
}
