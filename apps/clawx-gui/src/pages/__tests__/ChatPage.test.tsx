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

describe("ChatPage welcome-page send", () => {
  it("streams delta events into the UI when sending from the welcome page", async () => {
    const api = await import("../../lib/api");

    // listConversations empty at mount; createConversation returns a new one; listMessages returns empty initially then final
    (api.listConversations as any).mockReset();
    (api.listConversations as any).mockResolvedValue([]);

    (api.createConversation as any).mockReset();
    (api.createConversation as any).mockResolvedValue({
      id: "c-new", agent_id: "a1", title: "", created_at: "", updated_at: "",
    });

    (api.listMessages as any).mockReset();
    let serverFlushed = false;
    (api.listMessages as any).mockImplementation(async () => {
      if (!serverFlushed) return [];
      return [
        { id: "m1", conversation_id: "c-new", role: "user", content: "对话", created_at: "" },
        { id: "m2", conversation_id: "c-new", role: "assistant", content: "Hello world!", created_at: "" },
      ];
    });

    // Capture the stream callbacks so we can drive them
    let capturedOnMessage: ((data: string) => void) | undefined;
    let capturedOnDone: (() => void) | undefined;
    (api.sendMessageStream as any).mockReset();
    (api.sendMessageStream as any).mockImplementation(
      (_c: string, _msg: string, onMessage: any, onDone: any) => {
        capturedOnMessage = onMessage;
        capturedOnDone = onDone;
        return new AbortController();
      },
    );

    const user = userEvent.setup();
    render(
      <MemoryRouter initialEntries={["/?agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route path="/" element={<ChatPage />} />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );

    // wait for welcome screen to appear (agent loaded)
    const chip = await screen.findByRole("button", { name: "对话" });

    // click suggestion chip → triggers createConversation + sendMessageStream
    await user.click(chip);

    await waitFor(() => expect(capturedOnMessage).toBeDefined());

    // simulate backend sending delta payloads (the actual wire format)
    capturedOnMessage!(JSON.stringify({ delta: "Hello " }));
    capturedOnMessage!(JSON.stringify({ delta: "world!" }));

    // delta text should appear streamed in the UI
    await waitFor(() =>
      expect(screen.getByText("Hello world!")).toBeInTheDocument(),
    );

    // simulate done → backend has persisted real assistant message
    serverFlushed = true;
    capturedOnDone?.();

    // final message rendered via listMessages refresh
    await waitFor(() =>
      expect(screen.getAllByText("Hello world!").length).toBeGreaterThan(0),
    );
  });
});

describe("ChatPage background streaming", () => {
  it("renders the live typing indicator + partial text when returning to a streaming conv", async () => {
    const { beginStream, appendDelta, __resetStreamStore } = await import(
      "../../lib/chat-stream-store"
    );
    __resetStreamStore();

    // Simulate: a stream for `c-bg` has been running all along and has
    // accumulated some deltas while the user was off doing something else.
    beginStream("c-bg");
    appendDelta("c-bg", "生成中…");

    const api = await import("../../lib/api");
    (api.listMessages as any).mockReset();
    (api.listMessages as any).mockResolvedValue([]);
    (api.listConversations as any).mockReset();
    (api.listConversations as any).mockResolvedValue([
      { id: "c-bg", agent_id: "a1", title: "", created_at: "", updated_at: "" },
    ]);

    render(
      <MemoryRouter initialEntries={["/?conv=c-bg&agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route path="/" element={<ChatPage />} />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );

    // Partial assistant text already in store renders instantly — no extra
    // network turn required.
    await waitFor(() =>
      expect(screen.getByText("生成中…")).toBeInTheDocument(),
    );
  });

  it("does not abort an in-flight stream when the user navigates to another conv", async () => {
    const api = await import("../../lib/api");
    const abortController = new AbortController();
    const abortSpy = vi.spyOn(abortController, "abort");

    (api.sendMessageStream as any).mockReset();
    (api.sendMessageStream as any).mockImplementation(() => abortController);

    (api.listMessages as any).mockReset();
    (api.listMessages as any).mockResolvedValue([]);

    function Harness({ conv }: { conv: string }) {
      return (
        <MemoryRouter initialEntries={[`/?conv=${conv}&agent=a1`]}>
          <AgentProvider>
            <Routes>
              <Route path="/" element={<ChatPage />} />
            </Routes>
          </AgentProvider>
        </MemoryRouter>
      );
    }

    const { rerender } = render(<Harness conv="c1" />);
    await waitFor(() => expect(screen.getByText("glm-4.6")).toBeInTheDocument());

    const user = userEvent.setup();
    const input = await screen.findByPlaceholderText("输入任何问题...");
    await user.type(input, "ping{enter}");
    await waitFor(() => expect(api.sendMessageStream).toHaveBeenCalled());

    // Switch to a different conv — mimicking the user clicking another agent
    // that has its own active conversation. This must NOT abort c1's stream.
    rerender(<Harness conv="c2" />);

    // Give React a tick to flush the conv-change effect.
    await new Promise((r) => setTimeout(r, 20));

    expect(abortSpy).not.toHaveBeenCalled();
  });
});
