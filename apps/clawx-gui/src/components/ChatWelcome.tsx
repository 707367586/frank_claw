import { Sparkles, MessageSquare, FileText, Code, Search, ChevronRight } from "lucide-react";
import type { Agent } from "../lib/types";

const TAGS = ["对话", "文件创建", "代码编写", "分析研究", "总结", "文献检索", "任务规划", "代码审查"];

const SUGGESTIONS = [
  { icon: MessageSquare, text: "智能分析业务流程并提出建议" },
  { icon: FileText, text: "快速生成高质量技术文档" },
  { icon: Code, text: "为移动端设计一个技术方案" },
  { icon: Search, text: "研究并汇总行业最新动态" },
];

interface Props {
  agent?: Agent;
  onSend?: (t: string) => void | Promise<void>;
}

export default function ChatWelcome({ agent, onSend }: Props) {
  const title = agent?.name ?? "ClawX";
  const subtitle = agent?.system_prompt?.slice(0, 80)
    || "选中一个 Agent 开始对话，或在下方输入问题。";

  const handleSuggest = async (text: string) => {
    if (onSend) await onSend(text);
  };

  return (
    <div className="chat-welcome">
      <div className="chat-welcome__hero">
        <div className="chat-welcome__icon"><Sparkles size={30} /></div>
        <h1 className="chat-welcome__title">{title}</h1>
        <p className="chat-welcome__subtitle">{subtitle}</p>
      </div>
      <div className="chat-welcome__tags">
        {TAGS.map((t) => (
          <button key={t} className="chat-welcome__tag" onClick={() => handleSuggest(t)}>
            {t}
          </button>
        ))}
      </div>
      <ul className="chat-welcome__suggestions">
        {SUGGESTIONS.map((s) => (
          <li key={s.text}>
            <button className="chat-welcome__suggestion" onClick={() => handleSuggest(s.text)}>
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
