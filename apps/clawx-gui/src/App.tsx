import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import ChatPage from "./pages/ChatPage";
import ConnectorsPage from "./pages/ConnectorsPage";
import SettingsPage from "./pages/SettingsPage";
import NavBar from "./components/NavBar";
import { ClawProvider } from "./lib/store";

export default function App() {
  return (
    <ClawProvider>
      <BrowserRouter>
        <div className="flex h-screen">
          <NavBar />
          <main className="flex-1 overflow-hidden">
            <Routes>
              <Route path="/" element={<ChatPage />} />
              <Route path="/connectors" element={<ConnectorsPage />} />
              <Route path="/settings" element={<SettingsPage />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </main>
        </div>
      </BrowserRouter>
    </ClawProvider>
  );
}
