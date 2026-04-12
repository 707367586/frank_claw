import { Outlet } from "react-router-dom";
import NavBar from "../components/NavBar";
import ListPanel from "../components/ListPanel";

export default function AppLayout() {
  return (
    <div className="app-layout">
      <NavBar />
      <ListPanel />
      <main className="content-area">
        <Outlet />
      </main>
    </div>
  );
}
