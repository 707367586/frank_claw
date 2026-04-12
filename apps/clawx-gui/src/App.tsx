import { createBrowserRouter, RouterProvider } from "react-router-dom";
import AppLayout from "./layouts/AppLayout";
import ChatPage from "./pages/ChatPage";
import ContactsPage from "./pages/ContactsPage";
import KnowledgePage from "./pages/KnowledgePage";
import TasksPage from "./pages/TasksPage";
import ConnectorsPage from "./pages/ConnectorsPage";
import SettingsPage from "./pages/SettingsPage";

const router = createBrowserRouter([
  {
    path: "/",
    element: <AppLayout />,
    children: [
      { index: true, element: <ChatPage /> },
      { path: "contacts", element: <ContactsPage /> },
      { path: "knowledge", element: <KnowledgePage /> },
      { path: "tasks", element: <TasksPage /> },
      { path: "connectors", element: <ConnectorsPage /> },
      { path: "settings", element: <SettingsPage /> },
    ],
  },
]);

function App() {
  return <RouterProvider router={router} />;
}

export default App;
