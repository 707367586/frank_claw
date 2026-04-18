import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
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

  // TODO: onDone should no-op after conversation changes — if the user
  // navigates away mid-stream, onDone can fire after unmount/switch and
  // overwrite the now-current conversation's messages. Follow-up: cancellation
  // check before setMessages in refreshMessages / onDone path.
  it("appends streamed text to messages on stream done", async () => {
    const api = await import("../../lib/api");

    // Until the server has persisted the assistant reply, listMessages returns
    // empty so pong-from-llm is NOT in the DOM yet. After the post-done refresh
    // switch below, it returns the final conversation containing the assistant
    // reply.
    let serverPersisted = false;
    (api.listMessages as any).mockReset();
    (api.listMessages as any).mockImplementation(async () =>
      serverPersisted
        ? [
            { id: "m1", conversation_id: "c1", role: "user", content: "ping", created_at: "" },
            { id: "m2", conversation_id: "c1", role: "assistant", content: "pong-from-llm", created_at: "" },
          ]
        : [],
    );
    (api.listConversations as any).mockResolvedValue([
      { id: "c1", agent_id: "a1", title: "", created_at: "", updated_at: "" },
    ]);

    let capturedOnDone: (() => void) | undefined;
    (api.sendMessageStream as any).mockReset();
    (api.sendMessageStream as any).mockImplementation(
      (_c: string, _msg: string, _onMsg: any, onDone: any) => {
        capturedOnDone = onDone;
        return new AbortController();
      },
    );

    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/?conv=c1&agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route path="/" element={<ChatPage />} />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );

    // wait for initial load
    await waitFor(() =>
      expect((api.listMessages as any)).toHaveBeenCalledWith("c1"),
    );

    // type + send via Enter
    const input = await screen.findByPlaceholderText("输入任何问题...");
    await user.type(input, "ping{enter}");

    // Flip the mock so the refresh triggered by onDone returns the final
    // assistant reply, then simulate SSE done.
    serverPersisted = true;
    expect(capturedOnDone).toBeDefined();
    // Contract: the assistant reply must not appear until onDone fires and
    // the subsequent refresh completes.
    expect(screen.queryByText("pong-from-llm")).not.toBeInTheDocument();
    capturedOnDone?.();

    await waitFor(() =>
      expect(screen.getByText("pong-from-llm")).toBeInTheDocument(),
    );
  });
});
