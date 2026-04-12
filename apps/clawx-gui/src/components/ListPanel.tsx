import { useLocation } from "react-router-dom";
import { Search } from "lucide-react";

const panelConfig: Record<string, { title: string; placeholder: string }> = {
  "/": { title: "Conversations", placeholder: "Search conversations..." },
  "/contacts": { title: "Contacts", placeholder: "Search contacts..." },
  "/knowledge": { title: "Knowledge Base", placeholder: "Search knowledge..." },
  "/tasks": { title: "Scheduled Tasks", placeholder: "Search tasks..." },
  "/connectors": { title: "Connectors", placeholder: "Search connectors..." },
  "/settings": { title: "Settings", placeholder: "Search settings..." },
};

function getConfig(pathname: string) {
  const key = "/" + (pathname.split("/")[1] || "");
  return (
    panelConfig[key === "/" ? "/" : key] ?? {
      title: "Navigation",
      placeholder: "Search...",
    }
  );
}

export default function ListPanel() {
  const location = useLocation();
  const config = getConfig(location.pathname);

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <h2 className="list-panel-title">{config.title}</h2>
      </div>
      <div className="list-panel-search">
        <Search size={14} className="search-icon" />
        <input
          type="text"
          className="search-input"
          aria-label={`${config.title} search`}
          placeholder={config.placeholder}
        />
      </div>
      <div className="list-panel-content">
        <p className="list-placeholder">No items yet</p>
      </div>
    </aside>
  );
}
