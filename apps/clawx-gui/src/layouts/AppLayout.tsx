import { useState } from "react";
import { Outlet, useLocation } from "react-router-dom";
import NavBar from "../components/NavBar";
import AgentSidebar from "../components/AgentSidebar";
import PermissionModal from "../components/PermissionModal";
import type { PermissionRequest } from "../components/PermissionModal";
import { AgentProvider } from "../lib/store";

const DEMO_REQUESTS: PermissionRequest[] = [
  { id: "1", type: "fs_write", target: "/workspace/output.txt", risk: "medium", description: "Agent 需要将处理结果写入工作目录下的输出文件。" },
];

const SIDEBAR_HIDDEN = ["/agents", "/skills", "/settings"];

export default function AppLayout() {
  const [showPermission, setShowPermission] = useState(false);
  const { pathname } = useLocation();
  const hideSidebar = SIDEBAR_HIDDEN.some((p) => pathname.startsWith(p));

  return (
    <AgentProvider>
      <div className={`app-shell ${hideSidebar ? "app-shell--no-sidebar" : ""}`}>
        <NavBar />
        {!hideSidebar && <AgentSidebar />}
        <main className="app-shell__main">
          <Outlet />
        </main>

        {showPermission && (
          <PermissionModal
            agentName="研究助手"
            requests={DEMO_REQUESTS}
            onApprove={() => setShowPermission(false)}
            onDenyAll={() => setShowPermission(false)}
            onClose={() => setShowPermission(false)}
          />
        )}
      </div>
    </AgentProvider>
  );
}
