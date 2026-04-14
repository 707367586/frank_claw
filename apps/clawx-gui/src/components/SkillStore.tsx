import { Wrench, Braces, Mic, PenTool, FileSearch, Languages } from "lucide-react";
import Badge from "./ui/Badge";
import Button from "./ui/Button";

const SKILLS = [
  { id: "1", icon: Braces,     name: "代码生成", desc: "根据需求自动生成高质量的代码，支持多种编程语言。", enabled: true,  uses: 136, ts: "3 小时前" },
  { id: "2", icon: FileSearch, name: "数据分析", desc: "处理和分析大规模数据集，提供可视化洞察。",       enabled: true,  uses: 203, ts: "2 小时前" },
  { id: "3", icon: Mic,        name: "语音识别", desc: "将音频内容转换为文字，支持多种语言识别。",       enabled: false, uses: 194, ts: "昨天" },
  { id: "4", icon: PenTool,    name: "文档撰写", desc: "自动生成结构化文档、报告和技术说明。",          enabled: true,  uses: 120, ts: "4 小时前" },
  { id: "5", icon: Languages,  name: "智能翻译", desc: "支持多语言互译，保持原文语境和风格。",          enabled: true,  uses: 87,  ts: "1 天前" },
  { id: "6", icon: Wrench,     name: "摘要总结", desc: "快速提取长文本主要内容，生成精炼摘要。",         enabled: true,  uses: 148, ts: "6 小时前" },
];

export default function SkillStore() {
  return (
    <div className="skill-grid">
      {SKILLS.map((s) => (
        <div key={s.id} className="skill-card">
          <div className="skill-card__head">
            <div className="skill-card__icon"><s.icon size={16} /></div>
            <span className="skill-card__name">{s.name}</span>
            <Badge tone={s.enabled ? "success" : "neutral"}>{s.enabled ? "已启用" : "未启用"}</Badge>
          </div>
          <p className="skill-card__desc">{s.desc}</p>
          <span className="skill-card__meta">调用 {s.uses} 次 · 最近使用 {s.ts}</span>
          <div className="skill-card__actions">
            <Button variant="default" size="sm">使用</Button>
            <Button variant="outline" size="sm">编辑</Button>
          </div>
        </div>
      ))}
    </div>
  );
}
