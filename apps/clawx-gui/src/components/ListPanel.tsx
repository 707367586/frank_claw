import { useState } from "react";
import { useLocation } from "react-router-dom";
import { Search } from "lucide-react";
import ConversationList from "./ConversationList";
import AgentList from "./AgentList";
import AgentForm from "./AgentForm";

const panelConfig: Record<string, { title: string; placeholder: string }> = {
  "/contacts": { title: "Contacts", placeholder: "Search contacts..." },
  "/knowledge": { title: "Knowledge Base", placeholder: "Search knowledge..." },
  "/tasks": { title: "Scheduled Tasks", placeholder: "Search tasks..." },
  "/connectors": { title: "Connectors", placeholder: "Search connectors..." },
  "/settings": { title: "Settings", placeholder: "Search settings..." },
};

function getConfig(pathname: string) {
  const key = "/" + (pathname.split("/")[1] || "");
  return panelConfig[key] ?? null;
}

export default function ListPanel() {
  const location = useLocation();
  const [showCreateForm, setShowCreateForm] = useState(false);

  const isChatRoute =
    location.pathname === "/" || location.pathname === "";
  const isContactsRoute = location.pathname === "/contacts";

  // Chat route uses the dedicated ConversationList component
  if (isChatRoute) {
    return <ConversationList />;
  }

  // Contacts route uses AgentList
  if (isContactsRoute) {
    return (
      <>
        <AgentList onCreateAgent={() => setShowCreateForm(true)} />
        {showCreateForm && (
          <AgentForm
            onSaved={() => {
              setShowCreateForm(false);
              // Force re-render by navigating to same route
              window.location.reload();
            }}
            onCancel={() => setShowCreateForm(false)}
          />
        )}
      </>
    );
  }

  const config = getConfig(location.pathname);

  return (
    <aside className="list-panel">
      <div className="list-panel-header">
        <h2 className="list-panel-title">{config?.title ?? "Navigation"}</h2>
      </div>
      <div className="list-panel-search">
        <Search size={14} className="search-icon" />
        <input
          type="text"
          className="search-input"
          aria-label={`${config?.title ?? "Navigation"} search`}
          placeholder={config?.placeholder ?? "Search..."}
        />
      </div>
      <div className="list-panel-content">
        <p className="list-placeholder">No items yet</p>
      </div>
    </aside>
  );
}
