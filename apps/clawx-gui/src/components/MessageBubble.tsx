import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

interface MessageBubbleProps {
  role: "user" | "assistant";
  content: string;
  thought?: boolean;
}

export default function MessageBubble({ role, content, thought }: MessageBubbleProps) {
  const isUser = role === "user";
  return (
    <div className={`msg ${isUser ? "msg--user" : "msg--assistant"}`}>
      {thought && <span className="msg__thought-label">Thinking</span>}
      <div className={`msg__bubble${thought ? " msg__bubble--thought" : ""}`}>
        {isUser
          ? <span>{content}</span>
          : <Markdown remarkPlugins={[remarkGfm]}>{content}</Markdown>
        }
      </div>
    </div>
  );
}
