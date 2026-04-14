import { useState, type KeyboardEvent } from "react";
import { Plus, Zap, ArrowUp, ChevronDown } from "lucide-react";
import IconButton from "./ui/IconButton";

interface Props {
  onSend: (text: string) => void;
  disabled?: boolean;
  model?: string;
  onPickModel?: () => void;
}

export default function ChatInput({ onSend, disabled, model = "Sonnet 4.6", onPickModel }: Props) {
  const [value, setValue] = useState("");
  function submit() {
    const t = value.trim();
    if (!t || disabled) return;
    onSend(t);
    setValue("");
  }
  function onKey(e: KeyboardEvent<HTMLInputElement>) {
    if (e.key === "Enter" && !e.shiftKey) { e.preventDefault(); submit(); }
  }
  return (
    <div className="chat-input">
      <IconButton icon={<Plus size={16} />} aria-label="附件" variant="ghost" size="sm" />
      <button className="chat-input__skill" type="button">
        <Zap size={14} />
        <span>技能</span>
      </button>
      <input
        className="chat-input__field"
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onKeyDown={onKey}
        placeholder="输入任何问题..."
        disabled={disabled}
      />
      <button className="chat-input__model" type="button" onClick={onPickModel}>
        <span>{model}</span>
        <ChevronDown size={14} />
      </button>
      <IconButton
        icon={<ArrowUp size={16} />}
        aria-label="发送"
        variant="default"
        size="sm"
        onClick={submit}
        disabled={disabled || !value.trim()}
      />
    </div>
  );
}
