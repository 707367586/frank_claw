import { useLocation, useNavigate } from "react-router-dom";
import {
  MessageSquare, Users, Bot, BookOpen, CalendarClock, Plug, Settings,
} from "lucide-react";
import IconButton from "./ui/IconButton";
import Avatar from "./ui/Avatar";

const navItems = [
  { icon: MessageSquare, label: "对话", path: "/" },
  { icon: Users,         label: "联系人", path: "/contacts" },
  { icon: Bot,           label: "Agent & Skill", path: "/agents" },
  { icon: BookOpen,      label: "知识库", path: "/knowledge" },
  { icon: CalendarClock, label: "定时任务", path: "/tasks" },
  { icon: Plug,          label: "渠道", path: "/connectors" },
];

export default function NavBar() {
  const location = useLocation();
  const navigate = useNavigate();
  const isActive = (p: string) => p === "/" ? location.pathname === "/" : location.pathname.startsWith(p);

  return (
    <nav className="nav-rail" aria-label="Main navigation">
      <div className="nav-rail__top">
        <Avatar size={32} rounded="md" bg="var(--primary)">ZC</Avatar>
      </div>
      <div className="nav-rail__items">
        {navItems.map((it) => (
          <IconButton
            key={it.path}
            icon={<it.icon size={18} />}
            aria-label={it.label}
            title={it.label}
            onClick={() => navigate(it.path)}
            variant="ghost"
            className={isActive(it.path) ? "is-active" : ""}
          />
        ))}
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
