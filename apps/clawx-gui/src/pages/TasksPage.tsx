import { useState, useEffect, useCallback, useMemo } from "react";
import { useSearchParams } from "react-router-dom";
import {
  Trash2,
  Pause,
  Play,
  Search,
  Check,
  X,
  Edit3,
  CalendarClock,
  Clock,
  Activity,
  CheckCircle2,
  XCircle,
  ArrowLeft,
  Save,
} from "lucide-react";
import {
  getTask,
  updateTask,
  deleteTask,
  pauseTask,
  resumeTask,
  listTasks,
  listTriggers,
  listRuns,
} from "../lib/api";
import { LIFECYCLE_COLORS, RUN_STATUS_COLORS } from "../lib/constants";
import { useAgents } from "../lib/store";
import type { Agent, Task, Trigger, Run } from "../lib/types";

const LIFECYCLE_LABELS: Record<Task["lifecycle_status"], string> = {
  active: "启用",
  paused: "已暂停",
  archived: "已归档",
};

function formatDate(dateStr: string | null): string {
  if (!dateStr) return "-";
  return new Date(dateStr).toLocaleDateString("zh-CN", {
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

type FilterTab = "all" | "active" | "paused" | "failed";

/* ============================
   Task List View (no task selected)
   ============================ */
function TaskListView() {
  const [, setSearchParams] = useSearchParams();
  const [tasks, setTasks] = useState<Task[]>([]);
  const { agents: agentList } = useAgents();
  const agents = useMemo(() => {
    const map = new Map<string, string>();
    for (const a of agentList) map.set(a.id, a.name);
    return map;
  }, [agentList]);
  const [runs, setRuns] = useState<Map<string, Run[]>>(new Map());
  const [triggers, setTriggers] = useState<Map<string, Trigger[]>>(new Map());
  const [search, setSearch] = useState("");
  const [filter, setFilter] = useState<FilterTab>("all");
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    (async () => {
      try {
        const taskData = await listTasks();
        setTasks(taskData);

        // Load triggers and runs for each task
        const triggerMap = new Map<string, Trigger[]>();
        const runMap = new Map<string, Run[]>();
        await Promise.all(
          taskData.map(async (t: Task) => {
            try {
              const [trig, r] = await Promise.all([
                listTriggers(t.id),
                listRuns(t.id),
              ]);
              triggerMap.set(t.id, trig);
              runMap.set(t.id, r);
            } catch {
              // non-blocking
            }
          }),
        );
        setTriggers(triggerMap);
        setRuns(runMap);
      } catch {
        // ignore
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const filtered = useMemo(() => {
    let list = tasks;
    // Apply filter
    if (filter === "active") list = list.filter((t) => t.lifecycle_status === "active");
    else if (filter === "paused") list = list.filter((t) => t.lifecycle_status === "paused");
    else if (filter === "failed") {
      // Show tasks that have recent failed runs
      list = list.filter((t) => {
        const taskRuns = runs.get(t.id) ?? [];
        return taskRuns.some((r) => r.status === "failed");
      });
    }
    // Apply search
    const q = search.toLowerCase();
    if (q) {
      list = list.filter(
        (t) =>
          t.name.toLowerCase().includes(q) ||
          t.goal.toLowerCase().includes(q) ||
          (agents.get(t.agent_id) ?? "").toLowerCase().includes(q),
      );
    }
    return list;
  }, [tasks, filter, search, agents, runs]);

  const getFirstTimeTriggerText = (taskId: string): string => {
    const trigs = triggers.get(taskId) ?? [];
    const timeTrig = trigs.find((t) => t.kind === "time");
    if (timeTrig && timeTrig.config.cron) return `触发条件: ${timeTrig.config.cron as string}`;
    if (trigs.length > 0) return `触发条件: ${trigs[0].kind}`;
    return "无触发器";
  };

  const getNextFire = (taskId: string): string => {
    const trigs = triggers.get(taskId) ?? [];
    for (const t of trigs) {
      if (t.next_fire_at) return formatDate(t.next_fire_at);
    }
    return "-";
  };

  const getLastFire = (taskId: string): string => {
    const trigs = triggers.get(taskId) ?? [];
    for (const t of trigs) {
      if (t.last_fired_at) return formatDate(t.last_fired_at);
    }
    return "-";
  };

  const getSuccessCount = (taskId: string): number => {
    const taskRuns = runs.get(taskId) ?? [];
    return taskRuns.filter((r) => r.status === "completed").length;
  };

  return (
    <div className="task-list-view">
      {/* Top bar */}
      <div className="page-top-bar">
        <div className="page-top-bar-left">
          <CalendarClock size={20} />
          <h2>定时任务</h2>
        </div>
        <div className="page-top-bar-right">
          <button className="btn-primary-pill">
            <Play size={16} /> 自动任务
          </button>
          <div className="page-search-box">
            <Search size={14} />
            <input
              placeholder="搜索任务..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              aria-label="Search tasks"
            />
          </div>
        </div>
      </div>

      {/* Filter tabs */}
      <div className="task-filter-tabs">
        {([
          ["all", "全部"],
          ["active", "运行中"],
          ["paused", "已暂停"],
          ["failed", "失败"],
        ] as [FilterTab, string][]).map(([key, label]) => (
          <button
            key={key}
            className={`task-filter-tab ${filter === key ? "active" : ""}`}
            onClick={() => setFilter(key)}
          >
            {label}
          </button>
        ))}
      </div>

      {/* Task cards */}
      <div className="task-list-cards">
        {loading && <p className="list-placeholder">Loading...</p>}
        {!loading && filtered.length === 0 && (
          <p className="list-placeholder">
            {search ? "没有匹配的任务" : "暂无任务"}
          </p>
        )}
        {filtered.map((task) => (
          <div
            key={task.id}
            className="task-list-card"
            onClick={() => setSearchParams({ task: task.id })}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "Enter") setSearchParams({ task: task.id });
            }}
          >
            <div className="task-list-card-top">
              <div className="task-list-card-avatar">
                <CalendarClock size={16} />
              </div>
              <span className="task-list-card-name">{task.name}</span>
              <span
                className="task-list-card-status"
                style={{
                  background: LIFECYCLE_COLORS[task.lifecycle_status],
                  color: task.lifecycle_status === "paused" ? "#1a1a2e" : "#fff",
                }}
              >
                {LIFECYCLE_LABELS[task.lifecycle_status]}
              </span>
            </div>
            <div className="task-list-card-trigger">
              {getFirstTimeTriggerText(task.id)}
            </div>
            <div className="task-list-card-meta">
              <span>上次执行: {getLastFire(task.id)}</span>
              <span>下次执行: {getNextFire(task.id)}</span>
              <span className="task-list-card-success">
                <CheckCircle2 size={12} /> {getSuccessCount(task.id)} 次成功
              </span>
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

/* ============================
   Task Detail View (task selected)
   ============================ */
function TaskDetailView({ taskId }: { taskId: string }) {
  const [, setSearchParams] = useSearchParams();

  const [task, setTask] = useState<Task | null>(null);
  const [agentName, setAgentName] = useState<string>("");
  const [triggers, setTriggers] = useState<Trigger[]>([]);
  const [runs, setRuns] = useState<Run[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [mutationError, setMutationError] = useState<string | null>(null);

  // Edit mode
  const [editing, setEditing] = useState(false);
  const [editName, setEditName] = useState("");
  const [editGoal, setEditGoal] = useState("");
  const [editMaxSteps, setEditMaxSteps] = useState(10);
  const [editTimeout, setEditTimeout] = useState(300);
  const [saving, setSaving] = useState(false);

  const { agents: agentsList } = useAgents();

  const loadTask = useCallback(async (id: string) => {
    setLoading(true);
    setError(null);
    try {
      const taskData = await getTask(id);
      setTask(taskData);
      const agent = agentsList.find((a: Agent) => a.id === taskData.agent_id);
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
  }, [agentsList]);

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
      setMutationError(err instanceof Error ? err.message : "Failed to pause");
    }
  }, [task]);

  const handleResume = useCallback(async () => {
    if (!task) return;
    setMutationError(null);
    try {
      const updated = await resumeTask(task.id);
      setTask(updated);
    } catch (err) {
      setMutationError(err instanceof Error ? err.message : "Failed to resume");
    }
  }, [task]);

  const handleDelete = useCallback(async () => {
    if (!task) return;
    const confirmed = window.confirm(`删除任务 "${task.name}"? 此操作不可撤销。`);
    if (!confirmed) return;
    try {
      await deleteTask(task.id);
      setSearchParams({});
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to delete");
    }
  }, [task, setSearchParams]);

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
      setEditing(false);
    } catch (err) {
      setMutationError(err instanceof Error ? err.message : "Failed to update");
    } finally {
      setSaving(false);
    }
  }, [task, editName, editGoal, editMaxSteps, editTimeout]);

  // Derived stats
  const totalRuns = runs.length;
  const successRuns = runs.filter((r) => r.status === "completed").length;
  const failedRuns = runs.filter((r) => r.status === "failed").length;

  // Cron text from first time trigger
  const cronTrigger = triggers.find((t) => t.kind === "time");
  const cronText = cronTrigger ? (cronTrigger.config.cron as string) ?? "-" : "-";
  const triggerKindText = cronTrigger ? "Cron 定时" : triggers.length > 0 ? triggers[0].kind : "-";
  const nextFireAt = triggers.find((t) => t.next_fire_at)?.next_fire_at ?? null;

  if (loading) {
    return <div className="empty-state"><p>Loading...</p></div>;
  }
  if (error) {
    return <div className="empty-state"><h2>Error</h2><p>{error}</p></div>;
  }
  if (!task) {
    return <div className="empty-state"><h2>Task not found</h2></div>;
  }

  return (
    <div className="task-detail">
      {/* Back button */}
      <button
        className="task-detail-back"
        onClick={() => setSearchParams({})}
        aria-label="Back to task list"
      >
        <ArrowLeft size={16} /> 返回列表
      </button>

      {/* Header */}
      <div className="task-detail-header">
        <div className="task-detail-header-left">
          <h2 className="task-detail-name">{task.name}</h2>
          <span
            className="task-detail-status-badge"
            style={{
              background: LIFECYCLE_COLORS[task.lifecycle_status],
              color: task.lifecycle_status === "paused" ? "#1a1a2e" : "#fff",
            }}
          >
            {LIFECYCLE_LABELS[task.lifecycle_status]}
          </span>
        </div>
        <div className="task-detail-actions">
          <button className="btn-secondary btn-sm" onClick={() => setEditing(!editing)}>
            <Edit3 size={14} /> 编辑
          </button>
          <button className="btn-primary btn-sm" onClick={task.lifecycle_status === "active" ? handlePause : handleResume}>
            {task.lifecycle_status === "active" ? (
              <><Pause size={14} /> 暂停</>
            ) : (
              <><Play size={14} /> 恢复</>
            )}
          </button>
          <button className="btn-danger-outline btn-sm" onClick={handleDelete}>
            <Trash2 size={14} /> 删除
          </button>
        </div>
      </div>

      {mutationError && <p className="form-error" style={{ margin: "0 0 12px" }}>{mutationError}</p>}

      {/* Edit form (inline, toggled) */}
      {editing && (
        <div className="task-detail-edit-form">
          <label className="form-label">
            名称
            <input type="text" className="form-input" value={editName} onChange={(e) => setEditName(e.target.value)} />
          </label>
          <label className="form-label">
            目标
            <textarea className="form-textarea" value={editGoal} onChange={(e) => setEditGoal(e.target.value)} rows={2} />
          </label>
          <div style={{ display: "flex", gap: 12 }}>
            <label className="form-label" style={{ flex: 1 }}>
              最大步数
              <input type="number" className="form-input" value={editMaxSteps} onChange={(e) => setEditMaxSteps(Number(e.target.value))} min={1} />
            </label>
            <label className="form-label" style={{ flex: 1 }}>
              超时 (秒)
              <input type="number" className="form-input" value={editTimeout} onChange={(e) => setEditTimeout(Number(e.target.value))} min={1} />
            </label>
          </div>
          <div className="form-actions">
            <button className="btn-secondary" onClick={() => setEditing(false)}>取消</button>
            <button className="btn-primary" onClick={handleSaveEdit} disabled={saving}>
              <Save size={14} /> {saving ? "保存中..." : "保存"}
            </button>
          </div>
        </div>
      )}

      {/* Basic info grid */}
      <div className="task-detail-info-grid">
        <div className="task-info-item">
          <span className="task-info-label">关联 Agent</span>
          <span className="task-info-value">{agentName}</span>
        </div>
        <div className="task-info-item">
          <span className="task-info-label">触发器类型</span>
          <span className="task-info-value">{triggerKindText}</span>
        </div>
        <div className="task-info-item">
          <span className="task-info-label">Cron 规则</span>
          <span className="task-info-value" style={{ fontFamily: "monospace" }}>{cronText}</span>
        </div>
        <div className="task-info-item">
          <span className="task-info-label">描述</span>
          <span className="task-info-value">{task.goal || "-"}</span>
        </div>
      </div>

      {/* Execution stats */}
      <div className="task-stats">
        <div className="task-stat-card">
          <Activity size={20} className="task-stat-icon" style={{ color: "#60a5fa" }} />
          <span className="task-stat-number">{totalRuns}</span>
          <span className="task-stat-label">总执行</span>
        </div>
        <div className="task-stat-card task-stat-success">
          <CheckCircle2 size={20} className="task-stat-icon" style={{ color: "#4ade80" }} />
          <span className="task-stat-number" style={{ color: "#4ade80" }}>{successRuns}</span>
          <span className="task-stat-label">成功</span>
        </div>
        <div className="task-stat-card task-stat-failed">
          <XCircle size={20} className="task-stat-icon" style={{ color: "#f87171" }} />
          <span className="task-stat-number" style={{ color: "#f87171" }}>{failedRuns}</span>
          <span className="task-stat-label">失败</span>
        </div>
      </div>

      {/* Next schedule */}
      {nextFireAt && (
        <div className="task-next-schedule">
          <Clock size={14} />
          <span>下次执行: {formatDate(nextFireAt)}</span>
        </div>
      )}

      {/* Run History table */}
      <div className="task-run-table-section">
        <h3 className="task-section-title">执行历史</h3>
        {runs.length === 0 ? (
          <p className="list-placeholder">暂无执行记录</p>
        ) : (
          <table className="task-run-table">
            <thead>
              <tr>
                <th>触发时间</th>
                <th>耗时</th>
                <th>状态</th>
                <th>产物</th>
              </tr>
            </thead>
            <tbody>
              {runs.map((run) => (
                <tr key={run.id}>
                  <td>{formatDate(run.started_at)}</td>
                  <td>{formatDuration(run.started_at, run.completed_at)}</td>
                  <td>
                    <span
                      className="task-run-status-badge"
                      style={{
                        background: RUN_STATUS_COLORS[run.status] + "22",
                        color: RUN_STATUS_COLORS[run.status],
                      }}
                    >
                      {run.status === "completed" && <Check size={12} />}
                      {run.status === "failed" && <X size={12} />}
                      {run.status === "completed" ? "成功" : run.status === "failed" ? "失败" : run.status}
                    </span>
                  </td>
                  <td>
                    {run.checkpoint && Object.keys(run.checkpoint).length > 0
                      ? `${Object.keys(run.checkpoint).length} items`
                      : "-"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </div>
    </div>
  );
}

/* ============================
   Main export: routes between list & detail
   ============================ */
export default function TasksPage() {
  const [searchParams] = useSearchParams();
  const taskId = searchParams.get("task");

  if (!taskId) {
    return <TaskListView />;
  }

  return <TaskDetailView taskId={taskId} />;
}
