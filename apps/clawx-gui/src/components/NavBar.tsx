import { useLocation, useNavigate } from "react-router-dom";
import {
  MessageSquare,
  Users,
  BookOpen,
  Clock,
  Link,
  Settings,
} from "lucide-react";

const navItems = [
  { icon: MessageSquare, label: "Chat", path: "/" },
  { icon: Users, label: "Contacts", path: "/contacts" },
  { icon: BookOpen, label: "Knowledge", path: "/knowledge" },
  { icon: Clock, label: "Tasks", path: "/tasks" },
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
    <nav className="nav-bar">
      <div className="nav-bar-top">
        <div className="nav-logo">C</div>
        {navItems.map((item) => (
          <button
            key={item.path}
            className={`nav-icon-btn ${isActive(item.path) ? "active" : ""}`}
            onClick={() => navigate(item.path)}
            title={item.label}
          >
            <item.icon size={20} />
          </button>
        ))}
      </div>
      <div className="nav-bar-bottom">
        <button
          className={`nav-icon-btn ${isActive("/settings") ? "active" : ""}`}
          onClick={() => navigate("/settings")}
          title="Settings"
        >
          <Settings size={20} />
        </button>
      </div>
    </nav>
  );
}
