import { Cpu, Shield, Palette, HeartPulse, Info, MessageCircle } from "lucide-react";

const SECTIONS = [
  { id: "model",    icon: Cpu,        label: "模型" },
  { id: "security", icon: Shield,     label: "安全" },
  { id: "look",     icon: Palette,    label: "外观与语言" },
  { id: "health",   icon: HeartPulse, label: "健康" },
];

const EXTRA = [
  { id: "about",    icon: Info,          label: "关于" },
  { id: "feedback", icon: MessageCircle, label: "反馈" },
];

interface Props { value: string; onChange: (id: string) => void }

export default function SettingsNav({ value, onChange }: Props) {
  return (
    <aside className="settings-nav">
      <header className="settings-nav__head"><span>设置</span></header>
      <ul>
        {SECTIONS.map((s) => (
          <li key={s.id}>
            <button className={`settings-nav__item ${value === s.id ? "is-active" : ""}`} onClick={() => onChange(s.id)}>
              <s.icon size={14} /><span>{s.label}</span>
            </button>
          </li>
        ))}
      </ul>
      <div className="settings-nav__divider" />
      <ul>
        {EXTRA.map((s) => (
          <li key={s.id}>
            <button className={`settings-nav__item ${value === s.id ? "is-active" : ""}`} onClick={() => onChange(s.id)}>
              <s.icon size={14} /><span>{s.label}</span>
            </button>
          </li>
        ))}
      </ul>
    </aside>
  );
}
