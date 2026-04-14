import IconButton from "./ui/IconButton";
import { Pencil } from "lucide-react";

const ROWS = [
  { agent: "编程助手", strategy: "固定模型", current: "Claude Opus 4" },
  { agent: "研究助手", strategy: "智能路由", current: "按任务自动选择" },
  { agent: "写作助手", strategy: "固定模型", current: "Claude Sonnet 4.6" },
];

export default function AgentModelAssignTable() {
  return (
    <div className="mm-table">
      <div className="mm-table__head">
        <span>Agent</span><span>模型策略</span><span>当前模型</span><span />
      </div>
      {ROWS.map((r) => (
        <div key={r.agent} className="mm-table__row">
          <span>{r.agent}</span>
          <span>{r.strategy}</span>
          <span>{r.current}</span>
          <IconButton icon={<Pencil size={12} />} aria-label={`编辑 ${r.agent}`} size="sm" variant="ghost" />
        </div>
      ))}
    </div>
  );
}
