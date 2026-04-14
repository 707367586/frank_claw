import { useLocation, useNavigate } from "react-router-dom";
import {
  MessageSquare,
  Users,
  BookOpen,
  CalendarClock,
  Link,
  Settings,
} from "lucide-react";

const navItems = [
  { icon: MessageSquare, label: "Chat", path: "/" },
  { icon: Users, label: "Agent & Skill", path: "/agents" },
  { icon: BookOpen, label: "Knowledge", path: "/knowledge" },
  { icon: CalendarClock, label: "Tasks", path: "/tasks" },
  { icon: Link, label: "Connectors", path: "/connectors" },
];

export default function NavBar() {
  const location = useLocation();
  const navigate = useNavigate();

  function isActive(path: string) {
    if (path === "/") return location.pathname === "/";
    return location.pathname.startsWith(path);
  }

  return (
    <nav className="nav-bar" aria-label="Main navigation">
      {/* Window controls placeholder (Tauri will render native ones) */}
      <div className="nav-window-controls">
        <span className="nav-dot nav-dot--close" />
        <span className="nav-dot nav-dot--minimize" />
        <span className="nav-dot nav-dot--maximize" />
      </div>

      {/* Avatar */}
      <div className="nav-avatar-section">
        <div className="nav-avatar">周</div>
      </div>

      {/* Main nav icons */}
      <div className="nav-icons">
        {navItems.map((item) => (
          <button
            key={item.path}
            className={`nav-icon-btn ${isActive(item.path) ? "active" : ""}`}
            onClick={() => navigate(item.path)}
            title={item.label}
            aria-label={item.label}
          >
            <item.icon size={20} />
          </button>
        ))}
      </div>

      {/* Bottom: Settings */}
      <div className="nav-bottom">
        <button
          className={`nav-icon-btn ${isActive("/settings") ? "active" : ""}`}
          onClick={() => navigate("/settings")}
          title="Settings"
          aria-label="Settings"
        >
          <Settings size={20} />
        </button>
      </div>
    </nav>
  );
}
