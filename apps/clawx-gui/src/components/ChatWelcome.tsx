import {
  Bot, MessageSquare, FileText, Lightbulb, ChevronRight,
  Code2, Search, PenTool, BarChart3, Sparkles, Database, Globe, Wrench,
} from "lucide-react";
import type { ComponentType } from "react";
import type { Agent } from "../lib/agents-rest";

const ICONS: Record<string, ComponentType<{ size?: number }>> = {
  Code2, Search, PenTool, BarChart3, Bot, MessageSquare,
  FileText, Lightbulb, Sparkles, Database, Globe, Wrench,
};

const TAGS = ["对话", "文件处理", "任务规划", "代码生成"];
const SUGGESTIONS = [
  { icon: MessageSquare, text: "帮我分析这段代码的性能问题" },
  { icon: FileText,      text: "将这份报告整理为周报格式" },
  { icon: Lightbulb,     text: "为新功能设计一个技术方案" },
];

interface Props { agent?: Agent | null }

export default function ChatWelcome({ agent }: Props) {
  const Icon = (agent && ICONS[agent.icon]) || Bot;
  const title = agent?.name ?? "MaxClaw";
  const subtitle =
    agent?.description ||
    "您的智能 AI 助手，擅长编程、研究和创意任务。随时提问或试下方的建议。";
  const heroBg = agent?.color ?? "var(--primary)";

  return (
    <div data-testid="chat-welcome" className="chat-welcome">
      <div className="chat-welcome__hero">
        <div className="chat-welcome__icon" style={{ background: heroBg }}>
          <Icon size={30} />
        </div>
        <h1 className="chat-welcome__title">{title}</h1>
        <p className="chat-welcome__subtitle">{subtitle}</p>
      </div>
      <div className="chat-welcome__tags">
        {TAGS.map((t) => (
          <button key={t} type="button" className="chat-welcome__tag">{t}</button>
        ))}
      </div>
      <ul className="chat-welcome__suggestions">
        {SUGGESTIONS.map((s) => (
          <li key={s.text}>
            <button type="button" className="chat-welcome__suggestion">
              <s.icon size={16} className="chat-welcome__suggestion-icon" />
              <span>{s.text}</span>
              <ChevronRight size={14} className="chat-welcome__suggestion-chevron" />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
