import { useState, type FormEvent, type KeyboardEvent } from "react";
import { Plus, Zap, ArrowUp, ChevronDown } from "lucide-react";
import IconButton from "./ui/IconButton";

interface ChatInputProps {
  onSubmit: (text: string) => void;
  disabled?: boolean;
}

export default function ChatInput({ onSubmit, disabled }: ChatInputProps) {
  const [value, setValue] = useState("");

  function submit(e?: FormEvent) {
    e?.preventDefault();
    const t = value.trim();
    if (!t || disabled) return;
    onSubmit(t);
    setValue("");
  }

  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  }

  return (
    <form className="chat-input" onSubmit={submit}>
      <IconButton icon={<Plus size={16} />} aria-label="附件" variant="ghost" size="sm" />
      <button className="chat-input__skill" type="button">
        <Zap size={14} />
        <span>技能</span>
      </button>
      <input
        type="text"
        className="chat-input__field"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={onKey}
        placeholder="输入任何问题..."
        disabled={disabled}
      />
      <button className="chat-input__model" type="button" aria-label="选择模型">
        <span>Sonnet 4.6</span>
        <ChevronDown size={12} />
      </button>
      <IconButton
        icon={<ArrowUp size={16} />}
        aria-label="发送"
        variant="default"
        size="sm"
        onClick={() => submit()}
        disabled={disabled || !value.trim()}
      />
    </form>
  );
}
