import { useState } from "react";
import { Outlet } from "react-router-dom";
import NavBar from "../components/NavBar";
import AgentSidebar from "../components/AgentSidebar";
import PermissionModal from "../components/PermissionModal";
import type { PermissionRequest } from "../components/PermissionModal";
import { AgentProvider } from "../lib/store";

const DEMO_REQUESTS: PermissionRequest[] = [
  { id: "1", type: "fs_write", target: "/workspace/output.txt", risk: "medium", description: "Agent 需要将处理结果写入工作目录下的输出文件。" },
  { id: "2", type: "fs_delete", target: "/workspace/tmp/cache", risk: "high", description: "Agent 请求删除临时缓存目录。" },
  { id: "3", type: "net_http", target: "https://api.example.com", risk: "low", description: "Agent 请求访问外部 API 获取数据。" },
  { id: "4", type: "exec_shell", target: "npm run build", risk: "high", description: "Agent 请求执行 shell 命令进行项目构建。" },
];

export default function AppLayout() {
  const [showPermission, setShowPermission] = useState(false);

  return (
    <AgentProvider>
      <div className="app-layout">
        <NavBar />
        <AgentSidebar />
        <main className="main-content">
          <Outlet />
        </main>

        {showPermission && (
          <PermissionModal
            agentName="研究助手"
            requests={DEMO_REQUESTS}
            onApprove={(approved, denied) => {
              console.log("Approved:", approved, "Denied:", denied);
              setShowPermission(false);
            }}
            onDenyAll={() => {
              console.log("All denied");
              setShowPermission(false);
            }}
            onClose={() => setShowPermission(false)}
          />
        )}
      </div>
    </AgentProvider>
  );
}
