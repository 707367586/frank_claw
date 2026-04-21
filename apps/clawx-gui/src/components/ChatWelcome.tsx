import { Sparkles, MessageSquare, FileText, Code, Search, ChevronRight } from "lucide-react";

const SUGGESTIONS = [
  { icon: MessageSquare, text: "智能分析业务流程并提出建议" },
  { icon: FileText, text: "快速生成高质量技术文档" },
  { icon: Code, text: "为移动端设计一个技术方案" },
  { icon: Search, text: "研究并汇总行业最新动态" },
];

export default function ChatWelcome() {
  return (
    <div data-testid="chat-welcome" className="chat-welcome">
      <div className="chat-welcome__hero">
        <div className="chat-welcome__icon"><Sparkles size={30} /></div>
        <h1 className="chat-welcome__title">ClawX</h1>
        <p className="chat-welcome__subtitle">Type below to begin chatting.</p>
      </div>
      <ul className="chat-welcome__suggestions">
        {SUGGESTIONS.map((s) => (
          <li key={s.text}>
            <div className="chat-welcome__suggestion">
              <s.icon size={16} className="chat-welcome__suggestion-icon" />
              <span>{s.text}</span>
              <ChevronRight size={14} className="chat-welcome__suggestion-chevron" />
            </div>
          </li>
        ))}
      </ul>
    </div>
  );
}
