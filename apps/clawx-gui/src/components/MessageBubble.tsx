import { useState, useCallback } from "react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import hljs from "highlight.js";
import DOMPurify from "dompurify";
import { Copy, Check } from "lucide-react";
import type { Components } from "react-markdown";
import type { Message } from "../lib/types";

interface Props {
  message: Message;
  agentName?: string;
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

function getRelativeTime(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const diffSec = Math.floor((now - then) / 1000);
  if (diffSec < 60) return "just now";
  const diffMin = Math.floor(diffSec / 60);
  if (diffMin < 60) return `${diffMin} min ago`;
  const diffHr = Math.floor(diffMin / 60);
  if (diffHr < 24) return `${diffHr}h ago`;
  const diffDay = Math.floor(diffHr / 24);
  return `${diffDay}d ago`;
}

/** Extract step indicator like "Step 1/4" from the beginning of content */
function parseStepIndicator(content: string): { step: string; total: string } | null {
  const match = /^Step\s+(\d+)\/(\d+)/i.exec(content.trim());
  if (match) return { step: match[1], total: match[2] };
  return null;
}

export default function MessageBubble({ message, agentName }: Props) {
  const isUser = message.role === "user";
  const isSystem = message.role === "system";
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText(message.content).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    });
  }, [message.content]);

  if (isSystem) {
    return (
      <div className="message system">
        <p>{message.content}</p>
      </div>
    );
  }

  const stepInfo = !isUser ? parseStepIndicator(message.content) : null;

  return (
    <div className={`message-row ${message.role}`}>
      {/* Assistant avatar */}
      {!isUser && (
        <div className="message-avatar" title={agentName || "Assistant"}>
          {(agentName || "A").charAt(0).toUpperCase()}
        </div>
      )}

      <div className={`message-bubble ${message.role}`}>
        {/* Step badge */}
        {stepInfo && (
          <span className="step-badge">Step {stepInfo.step}/{stepInfo.total}</span>
        )}

        {/* Copy button on hover */}
        <button
          className="message-copy-btn"
          onClick={handleCopy}
          aria-label="Copy message"
          title="Copy"
        >
          {copied ? <Check size={14} /> : <Copy size={14} />}
        </button>

        {/* Timestamp on hover */}
        <span className="message-timestamp">
          {getRelativeTime(message.created_at)}
        </span>

        {/* Content */}
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
    </div>
  );
}
