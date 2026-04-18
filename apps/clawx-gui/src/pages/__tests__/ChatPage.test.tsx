import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MemoryRouter, Routes, Route } from "react-router-dom";
import ChatPage from "../ChatPage";
import { AgentProvider } from "../../lib/store";

vi.mock("../../lib/api", () => ({
  listAgents: vi.fn().mockResolvedValue([
    {
      id: "a1", name: "编程助手", role: "Developer",
      system_prompt: "helper", model_id: "p1",
      status: "idle", created_at: "", updated_at: "",
    },
  ]),
  listModels: vi.fn().mockResolvedValue([
    { id: "p1", name: "智谱", provider_type: "zhipu",
      base_url: "", model_name: "glm-4.6", parameters: {},
      is_default: true, created_at: "", updated_at: "" },
  ]),
  listConversations: vi.fn().mockResolvedValue([
    { id: "c1", agent_id: "a1", title: "对话", created_at: "", updated_at: "" },
  ]),
  listMessages: vi.fn().mockResolvedValue([]),
  sendMessageStream: vi.fn(() => new AbortController()),
  createConversation: vi.fn(),
}));

describe("ChatPage model surface", () => {
  it("shows the model bound to the selected agent", async () => {
    render(
      <MemoryRouter initialEntries={["/?conv=c1&agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route path="/" element={<ChatPage />} />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("glm-4.6")).toBeInTheDocument());
  });

  it("shows 未选择 while provider list is still loading", async () => {
    const { listModels } = await import("../../lib/api");
    (listModels as unknown as ReturnType<typeof vi.fn>).mockImplementationOnce(
      () => new Promise(() => {}),
    );

    render(
      <MemoryRouter initialEntries={["/?conv=c1&agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route path="/" element={<ChatPage />} />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByText("未选择")).toBeInTheDocument());
  });

  it("falls back to 未选择 when agent's model_id has no matching provider", async () => {
    const api = await import("../../lib/api");
    (api.listModels as unknown as ReturnType<typeof vi.fn>).mockResolvedValueOnce([
      { id: "different-id", name: "x", provider_type: "zhipu",
        base_url: "", model_name: "glm-4.6", parameters: {},
        is_default: false, created_at: "", updated_at: "" },
    ]);

    render(
      <MemoryRouter initialEntries={["/?conv=c1&agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route path="/" element={<ChatPage />} />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByText("未选择")).toBeInTheDocument());
  });
});
