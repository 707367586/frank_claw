import { Outlet, useLocation } from "react-router-dom";
import NavBar from "../components/NavBar";
import AgentSidebar from "../components/AgentSidebar";
import { AgentProvider } from "../lib/store";

const SIDEBAR_HIDDEN = ["/agents", "/skills", "/settings"];

export default function AppLayout() {
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
      </div>
    </AgentProvider>
  );
}
