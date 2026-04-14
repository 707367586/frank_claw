import { Clock, Check, X, ThumbsDown } from "lucide-react";
import Badge from "./ui/Badge";

interface Task {
  id: string;
  name: string;
  agent: string;
  schedule: string;
  status: "running" | "paused" | "error";
  lastRun: string;
  feedback: { accepted: number; ignored: number; negative: number };
}

const STATUS_TONE = { running: "success", paused: "neutral", error: "error" } as const;
const STATUS_LABEL = { running: "运行中", paused: "已暂停", error: "出错" } as const;

export default function TaskCard({ task }: { task: Task }) {
  return (
    <li className="task-card">
      <header className="task-card__head">
        <Clock size={16} className="task-card__icon" />
        <span className="task-card__name">{task.name}</span>
        <Badge tone={STATUS_TONE[task.status]}>{STATUS_LABEL[task.status]}</Badge>
      </header>
      <div className="task-card__sub">{task.agent} · {task.schedule}</div>
      <div className="task-card__meta">上次执行: {task.lastRun}</div>
      <div className="task-card__feedback">
        <span className="fb fb--ok"><Check size={12} /> 采纳 {task.feedback.accepted} 次</span>
        <span className="fb fb--neutral"><X size={12} /> 忽略 {task.feedback.ignored} 次</span>
        <span className="fb fb--bad"><ThumbsDown size={12} /> 负反馈 {task.feedback.negative} 次</span>
      </div>
    </li>
  );
}
