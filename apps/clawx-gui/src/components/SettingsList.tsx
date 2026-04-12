import { useSearchParams } from "react-router-dom";
import { Cpu, Shield, Activity, Info } from "lucide-react";

const SETTINGS_CATEGORIES = [
  { key: "models", label: "Models", icon: Cpu },
  { key: "security", label: "Security", icon: Shield },
  { key: "system", label: "System", icon: Activity },
  { key: "about", label: "About", icon: Info },
] as const;

export default function SettingsList() {
  const [searchParams, setSearchParams] = useSearchParams();
  const selected = searchParams.get("section") ?? "models";

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <h2 className="list-panel-title">Settings</h2>
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
