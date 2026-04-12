import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import hljs from "highlight.js";
import DOMPurify from "dompurify";
import type { Components } from "react-markdown";
import type { Message } from "../lib/types";

interface Props {
  message: Message;
}

interface CodeProps {
  className?: string;
  children?: React.ReactNode;
}

// Extracted to module scope to avoid re-creating on every render
const markdownComponents: Components = {
  code({ className, children }: CodeProps) {
    const match = /language-(\w+)/.exec(className || "");
    const code = String(children).replace(/\n$/, "");
    if (match) {
      let highlighted: string;
      try {
        highlighted = hljs.highlight(code, {
          language: match[1],
        }).value;
      } catch {
        highlighted = hljs.highlightAuto(code).value;
      }
      return (
        <pre className="code-block">
          <code
            dangerouslySetInnerHTML={{
              __html: DOMPurify.sanitize(highlighted),
            }}
          />
        </pre>
      );
    }
    return <code className="inline-code">{children}</code>;
  },
};

export default function MessageBubble({ message }: Props) {
  const isUser = message.role === "user";

  return (
    <div className={`message ${message.role}`}>
      {isUser ? (
        <p>{message.content}</p>
      ) : (
        <div className="markdown-body">
          <ReactMarkdown
            remarkPlugins={[remarkGfm]}
            components={markdownComponents}
          >
            {message.content}
          </ReactMarkdown>
        </div>
      )}
    </div>
  );
}
