import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ChatPage from "../ChatPage";
import { ClawProvider } from "../../lib/store";

vi.mock("../../lib/pico-rest", () => ({
  fetchPicoInfo: vi.fn().mockResolvedValue({
    configured: true,
    enabled: true,
    ws_url: "ws://localhost:18800/pico/ws",
  }),
}));

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
    // Wait for token-bootstrap useEffects to settle
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
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

  it("shows 'Pico channel disabled' when info.enabled is false", async () => {
    const { fetchPicoInfo } = await import("../../lib/pico-rest");
    (fetchPicoInfo as unknown as { mockResolvedValue: (v: unknown) => void })
      .mockResolvedValue({ configured: true, enabled: false, ws_url: "ws://x" });
    render(
      <MemoryRouter>
        <ClawProvider>
          <ChatPage />
        </ClawProvider>
      </MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(screen.getByText(/Pico channel disabled/i)).toBeInTheDocument();
  });
});
