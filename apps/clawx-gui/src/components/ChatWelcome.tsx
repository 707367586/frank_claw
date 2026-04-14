import { Bot } from "lucide-react";
import ChatInput from "./ChatInput";

interface ChatWelcomeProps {
  agent: { id: string; name: string; role: string };
  onSend: (text: string) => void;
}

const QUICK_TAGS = [
  "问答", "总结提炼", "代码辅助", "科研", "知识", "任务助手", "内容创作", "性能优化",
];

const SUGGESTIONS_BY_ROLE: Record<string, string[]> = {
  developer: [
    "帮我分析这段代码的性能问题",
    "写一个 REST API 的单元测试",
    "帮我重构这个函数，提升可读性",
  ],
  researcher: [
    "帮我总结这篇论文的核心观点",
    "对比分析这两种技术方案的优劣",
    "帮我梳理这个领域的研究现状",
  ],
  writer: [
    "帮我润色这段文字",
    "根据大纲生成一篇技术博客",
    "帮我写一份项目周报",
  ],
  default: [
    "帮我分析这段代码的性能问题",
    "总结一下今天的工作进展",
    "帮我梳理一个技术方案",
  ],
};

function getSuggestions(role: string): string[] {
  const key = role.toLowerCase();
  for (const [k, v] of Object.entries(SUGGESTIONS_BY_ROLE)) {
    if (key.includes(k)) return v;
  }
  return SUGGESTIONS_BY_ROLE.default;
}

export default function ChatWelcome({ agent, onSend }: ChatWelcomeProps) {
  const suggestions = getSuggestions(agent.role);

  return (
    <div className="chat-welcome">
      {/* Top bar with tabs */}
      <div className="chat-top-bar">
        <div className="chat-tabs">
          <button className="chat-tab active">Conversation</button>
          <button className="chat-tab">Artifacts</button>
        </div>
      </div>

      {/* Center content */}
      <div className="chat-welcome-center">
        <div className="chat-welcome-icon">
          <Bot size={32} />
        </div>
        <h1 className="chat-welcome-name">{agent.name}</h1>
        <p className="chat-welcome-desc">
          Your intelligent AI assistant for coding, research, and creative tasks. Ask me anything or try one of the suggestions below.
        </p>
        <div className="chat-welcome-tags">
          {QUICK_TAGS.map((tag) => (
            <button key={tag} className="chat-welcome-tag" onClick={() => onSend(tag)}>
              {tag}
            </button>
          ))}
        </div>
        <div className="chat-welcome-suggestions">
          {suggestions.map((text) => (
            <button
              key={text}
              className="chat-welcome-suggestion"
              onClick={() => onSend(text)}
            >
              <span className="suggestion-checkbox">☐</span>
              {text}
            </button>
          ))}
        </div>
      </div>

      {/* Input bar */}
      <ChatInput onSend={onSend} />
    </div>
  );
}
