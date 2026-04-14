import { useState, useCallback, useRef } from "react";
import { ArrowUp, Plus } from "lucide-react";

interface Props {
  onSend: (content: string) => void;
  disabled?: boolean;
  agentName?: string;
}

const MAX_LINES = 5;
const LINE_HEIGHT = 20;
const BASE_HEIGHT = 40; // single-line height (padding included)

export default function ChatInput({ onSend, disabled, agentName }: Props) {
  const [text, setText] = useState("");
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const tokenCount = Math.ceil(text.length / 4);

  const handleSend = useCallback(() => {
    const trimmed = text.trim();
    if (!trimmed || disabled) return;
    onSend(trimmed);
    setText("");
    // Reset textarea height after sending
    if (textareaRef.current) {
      textareaRef.current.style.height = `${BASE_HEIGHT}px`;
    }
  }, [text, disabled, onSend]);

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        handleSend();
      }
      // Shift+Enter: default behavior (new line)
    },
    [handleSend],
  );

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      setText(e.target.value);
      // Auto-resize
      const el = e.target;
      el.style.height = "auto";
      const maxHeight = LINE_HEIGHT * MAX_LINES + 20; // 20px for padding
      el.style.height = `${Math.min(el.scrollHeight, maxHeight)}px`;
    },
    [],
  );

  const placeholder = agentName
    ? `Ask ${agentName} Anything...`
    : "Type a message...";

  return (
    <div className="input-bar">
      <button
        className="input-attach-btn"
        aria-label="Attach file"
        title="Attach file"
        type="button"
      >
        <Plus size={18} />
      </button>

      <textarea
        ref={textareaRef}
        value={text}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        placeholder={placeholder}
        disabled={disabled}
        rows={1}
        aria-label="Message input"
        className="input-textarea"
      />

      <div className="input-right">
        <span className="input-token-count">{tokenCount}/8K</span>
        <button
          className="input-send-btn"
          onClick={handleSend}
          disabled={disabled || !text.trim()}
          aria-label="Send message"
        >
          <ArrowUp size={16} />
        </button>
      </div>
    </div>
  );
}
