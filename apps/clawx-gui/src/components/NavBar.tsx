import { useLocation, useNavigate } from "react-router-dom";
import { MessageSquare, Users, Database, Clock, Plug, Bot, Settings } from "lucide-react";
import IconButton from "./ui/IconButton";
import Avatar from "./ui/Avatar";

// Routes registered in App.tsx. Items whose `path` is in this set navigate;
// the rest are visual placeholders (no-op click) until their pages exist.
const ROUTED_PATHS = new Set(["/", "/connectors", "/settings"]);

const navItems = [
  { icon: MessageSquare, label: "对话",   path: "/" },
  { icon: Users,         label: "联系人", path: "/contacts" },
  { icon: Database,      label: "知识库", path: "/knowledge" },
  { icon: Clock,         label: "任务",   path: "/tasks" },
  { icon: Plug,          label: "渠道",   path: "/connectors" },
  { icon: Bot,           label: "Agent",  path: "/agents" },
];

export default function NavBar() {
  const location = useLocation();
  const navigate = useNavigate();
  const isActive = (p: string) =>
    p === "/" ? location.pathname === "/" : location.pathname.startsWith(p);

  return (
    <nav className="nav-rail" aria-label="Main navigation">
      <div className="nav-rail__top">
        <Avatar size={32} rounded="md" bg="var(--primary)">周</Avatar>
      </div>
      <div className="nav-rail__items">
        {navItems.map((it) => {
          const routed = ROUTED_PATHS.has(it.path);
          return (
            <IconButton
              key={it.path}
              icon={<it.icon size={18} />}
              aria-label={it.label}
              title={routed ? it.label : `${it.label}（即将推出）`}
              onClick={() => { if (routed) navigate(it.path); }}
              variant="ghost"
              className={routed && isActive(it.path) ? "is-active" : ""}
            />
          );
        })}
      </div>
      <div className="nav-rail__bottom">
        <IconButton
          icon={<Settings size={18} />}
          aria-label="设置"
          title="设置"
          onClick={() => navigate("/settings")}
          variant="ghost"
          className={isActive("/settings") ? "is-active" : ""}
        />
      </div>
    </nav>
  );
}
