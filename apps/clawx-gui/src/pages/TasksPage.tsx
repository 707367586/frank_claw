import { useState } from "react";
import { Plus, Search } from "lucide-react";
import Button from "../components/ui/Button";
import Input from "../components/ui/Input";
import { TabsRoot, TabsList, TabsTrigger } from "../components/ui/Tabs";
import TaskCard from "../components/TaskCard";

type TaskStatus = "running" | "paused" | "error";
const MOCK: { id: string; name: string; agent: string; schedule: string; status: TaskStatus; lastRun: string; feedback: { accepted: number; ignored: number; negative: number } }[] = [
  { id: "1", name: "每日晨报生成", agent: "编程助手", schedule: "每天 08:00",  status: "running", lastRun: "今天 08:00 · 成功", feedback: { accepted: 12, ignored: 2, negative: 0 } },
  { id: "2", name: "竞品监控周报", agent: "研究助手", schedule: "每周一 09:00", status: "paused",  lastRun: "3月24日 09:00 · 成功", feedback: { accepted: 8, ignored: 0, negative: 1 } },
  { id: "3", name: "PR 合并后自动更新文档", agent: "编程助手", schedule: "事件触发: GitHub webhook", status: "running", lastRun: "今天 14:22 · 成功", feedback: { accepted: 3, ignored: 1, negative: 0 } },
];

export default function TasksPage() {
  const [tab, setTab] = useState("all");
  const [q, setQ] = useState("");
  const list = MOCK.filter((t) => {
    if (tab === "all") return true;
    if (tab === "running") return t.status === "running";
    if (tab === "paused") return t.status === "paused";
    return t.status === "error";
  }).filter((t) => !q || t.name.includes(q));

  return (
    <div className="tasks-page">
      <header className="tasks-page__head">
        <h1>定时任务</h1>
        <Button leftIcon={<Plus size={14} />} size="sm">创建任务</Button>
      </header>

      <div className="tasks-page__bar">
        <Input size="sm" leftIcon={<Search size={14} />} placeholder="搜索任务..." value={q} onChange={(e) => setQ(e.target.value)} />
        <TabsRoot value={tab} onChange={setTab}>
          <TabsList>
            <TabsTrigger value="all">全部</TabsTrigger>
            <TabsTrigger value="running">运行中</TabsTrigger>
            <TabsTrigger value="paused">已暂停</TabsTrigger>
            <TabsTrigger value="error">出错</TabsTrigger>
          </TabsList>
        </TabsRoot>
      </div>

      <ul className="tasks-page__list">
        {list.map((t) => <TaskCard key={t.id} task={t} />)}
      </ul>
    </div>
  );
}
