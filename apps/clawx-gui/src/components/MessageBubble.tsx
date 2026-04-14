import type { Message } from "../lib/types";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";

export default function MessageBubble({ message }: { message: Message }) {
  const isUser = message.role === "user";
  return (
    <div className={`msg ${isUser ? "msg--user" : "msg--assistant"}`}>
      <div className="msg__bubble">
        {isUser ? <span>{message.content}</span> : <Markdown remarkPlugins={[remarkGfm]}>{message.content}</Markdown>}
      </div>
    </div>
  );
}
