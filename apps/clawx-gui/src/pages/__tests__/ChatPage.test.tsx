import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ChatPage from "../ChatPage";
import { ClawProvider } from "../../lib/store";

vi.mock("../../lib/agents-rest", () => ({
  listAgents: vi.fn().mockResolvedValue([
    {
      id: "a1", name: "default", description: "", color: "#5749F4", icon: "Bot",
      system_prompt: "", model: null, enabled_toolsets: [],
      workspace_dir: "/tmp/a1", current_session_id: "sid-1", created_at: 1,
    },
  ]),
  listToolsets: vi.fn().mockResolvedValue([]),
  createAgent: vi.fn(),
  deleteAgent: vi.fn().mockResolvedValue(undefined),
  rotateAgentSession: vi.fn().mockResolvedValue({ session_id: "sid-new" }),
}));

vi.mock("../../lib/hermes-rest", async () => {
  const actual = await vi.importActual<typeof import("../../lib/hermes-rest")>("../../lib/hermes-rest");
  return {
    ...actual,
    fetchHermesInfo: vi.fn().mockResolvedValue({
      configured: true,
      enabled: true,
      ws_url: "ws://localhost:18800/hermes/ws",
      provider: null,
      missing_env_var: null,
    }),
    getSession: vi.fn().mockResolvedValue({
      id: "sid-1", title: "", preview: "", message_count: 0,
      created: 0, updated: 0, summary: "",
      messages: [],
    }),
  };
});

class FakeWS {
  static last: FakeWS;
  readyState = 1;
  onopen?: () => void;
  onmessage?: (e: { data: string }) => void;
  onclose?: () => void;
  onerror?: () => void;
  sent: string[] = [];
  constructor(public url: string, public protocols?: string | string[]) {
    FakeWS.last = this;
    queueMicrotask(() => this.onopen?.());
  }
  send(d: string) { this.sent.push(d); }
  close() { this.onclose?.(); }
}
beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("clawx.dashboard_token", "T");
  vi.stubGlobal("WebSocket", FakeWS as unknown as typeof WebSocket);
  (FakeWS as unknown as { OPEN: number }).OPEN = 1;
});

describe("ChatPage", () => {
  it("welcome → user send → assistant reply renders", async () => {
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    // Wait for token-bootstrap useEffects to settle (agents + wsUrl + WS connect)
    await act(async () => { await new Promise((r) => setTimeout(r, 50)); });
    expect(screen.getByTestId("chat-welcome")).toBeInTheDocument();

    const input = screen.getByRole("textbox");
    await act(async () => {
      fireEvent.change(input, { target: { value: "hello" } });
      fireEvent.submit(input.closest("form")!);
    });
    expect(await screen.findByText("hello")).toBeInTheDocument();

    await act(async () => {
      FakeWS.last.onmessage?.({
        data: JSON.stringify({
          type: "message.create",
          payload: { message_id: "a1", content: "hi back" },
        }),
      });
    });
    expect(await screen.findByText("hi back")).toBeInTheDocument();
  });

  it("shows 'Hermes is not configured' when info.enabled is false", async () => {
    const { fetchHermesInfo } = await import("../../lib/hermes-rest");
    (fetchHermesInfo as unknown as { mockResolvedValue: (v: unknown) => void })
      .mockResolvedValue({
        configured: true,
        enabled: false,
        ws_url: "ws://x",
        provider: null,
        missing_env_var: null,
      });
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(screen.getByText(/Hermes is not configured/i)).toBeInTheDocument();
  });

  it("shows missing env var when backend reports one", async () => {
    const { fetchHermesInfo } = await import("../../lib/hermes-rest");
    (fetchHermesInfo as unknown as { mockResolvedValue: (v: unknown) => void })
      .mockResolvedValue({
        configured: false,
        enabled: false,
        ws_url: "ws://x",
        provider: "zai",
        missing_env_var: "GLM_API_KEY",
      });
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(screen.getAllByText(/GLM_API_KEY/).length).toBeGreaterThan(0);
    expect(screen.getByText(/~\/\.hermes\/\.env/)).toBeInTheDocument();
  });

  it("falls back to generic message when no hint available", async () => {
    const { fetchHermesInfo } = await import("../../lib/hermes-rest");
    (fetchHermesInfo as unknown as { mockResolvedValue: (v: unknown) => void })
      .mockResolvedValue({
        configured: false,
        enabled: false,
        ws_url: "ws://x",
        provider: null,
        missing_env_var: null,
      });
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(screen.getByText(/init_config\.py/)).toBeInTheDocument();
  });

  it("shows '新对话' button next to tabs", async () => {
    const { fetchHermesInfo } = await import("../../lib/hermes-rest");
    (fetchHermesInfo as unknown as { mockResolvedValue: (v: unknown) => void })
      .mockResolvedValue({
        configured: true,
        enabled: true,
        ws_url: "ws://localhost:18800/hermes/ws",
        provider: null,
        missing_env_var: null,
      });
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 50)); });
    expect(screen.getByRole("button", { name: /新对话/ })).toBeInTheDocument();
  });
});
