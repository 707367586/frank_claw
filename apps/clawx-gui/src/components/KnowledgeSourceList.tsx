import { FolderOpen, BookOpen, FileCode, Mic, Plus, MessagesSquare } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import Badge from "./ui/Badge";
import Button from "./ui/Button";
import Progress from "./ui/Progress";

interface Source { id: string; icon: LucideIcon; name: string; docs: number; status: "active" | "indexing"; progress?: number; group: "local" | "chat" }

const LOCAL: Source[] = [
  { id: "1", icon: FolderOpen, name: "产品文档库", docs: 23, status: "active",   group: "local" },
  { id: "2", icon: BookOpen,   name: "技术规范",   docs: 15, status: "active",   group: "local" },
  { id: "3", icon: FileCode,   name: "竞品分析",   docs: 8,  status: "indexing", progress: 72, group: "local" },
  { id: "4", icon: Mic,        name: "会议记录",   docs: 45, status: "active",   group: "local" },
];

const CHAT: Source[] = [
  { id: "c1", icon: MessagesSquare, name: "产品策略讨论",  docs: 12, status: "indexing", progress: 40, group: "chat" },
  { id: "c2", icon: MessagesSquare, name: "技术方案评审",  docs: 5,  status: "active",   group: "chat" },
  { id: "c3", icon: MessagesSquare, name: "竞品调研总结",  docs: 9,  status: "active",   group: "chat" },
];

function Row({ s }: { s: Source }) {
  const Icon = s.icon;
  return (
    <div className="kn-src">
      <div className="kn-src__icon"><Icon size={16} /></div>
      <div className="kn-src__body">
        <div className="kn-src__name">{s.name}</div>
        <div className="kn-src__meta">{s.docs} 篇文档 · {s.status === "active" ? "已索引" : `索引中 ${s.progress ?? 0}%`}</div>
        {s.status === "indexing" && <Progress value={s.progress ?? 0} />}
      </div>
      <Badge tone={s.status === "active" ? "success" : "warning"}>{s.status === "active" ? "活跃" : "索引中"}</Badge>
    </div>
  );
}

export default function KnowledgeSourceList() {
  return (
    <div className="kn-list">
      <header className="kn-list__head">
        <h2>知识库</h2>
        <Button leftIcon={<Plus size={14} />} size="sm">添加知识源</Button>
      </header>
      <section>
        <h3 className="kn-list__group">本地添加 ({LOCAL.length})</h3>
        {LOCAL.map((s) => <Row key={s.id} s={s} />)}
      </section>
      <section>
        <h3 className="kn-list__group">对话产生 ({CHAT.length})</h3>
        {CHAT.map((s) => <Row key={s.id} s={s} />)}
      </section>
    </div>
  );
}
