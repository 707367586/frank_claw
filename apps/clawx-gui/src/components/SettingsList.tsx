import { useSearchParams } from "react-router-dom";
import { Cpu, Shield, Palette, Activity, Info, MessageCircle } from "lucide-react";

const SETTINGS_CATEGORIES = [
  { key: "models", label: "模型", icon: Cpu },
  { key: "security", label: "安全", icon: Shield },
  { key: "appearance", label: "外观与语言", icon: Palette },
  { key: "system", label: "健康", icon: Activity },
  { key: "about", label: "关于", icon: Info },
  { key: "feedback", label: "反馈", icon: MessageCircle },
] as const;

export default function SettingsList() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selected = searchParams.get("section") ?? "models";

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <h2 className="settings-sidebar-title">Settings</h2>
      </div>
      <div className="list-panel-content">
        {SETTINGS_CATEGORIES.map((cat) => {
          const Icon = cat.icon;
          return (
            <button
              key={cat.key}
              className={`settings-category-item ${selected === cat.key ? "selected" : ""}`}
              onClick={() => setSearchParams({ section: cat.key })}
              aria-label={`Settings section ${cat.label}`}
            >
              <Icon size={16} className="settings-category-icon" />
              <span className="settings-category-label">{cat.label}</span>
            </button>
          );
        })}
      </div>
    </aside>
  );
}
