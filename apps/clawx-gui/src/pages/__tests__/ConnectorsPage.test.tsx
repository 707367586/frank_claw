import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import ConnectorsPage from "../ConnectorsPage";
import { ClawProvider } from "../../lib/store";

const mocks = vi.hoisted(() => ({
  fetchPicoInfo: vi.fn().mockResolvedValue({
    configured: true, enabled: true, ws_url: "ws://x",
  }),
  listSkills: vi.fn().mockResolvedValue([
    { name: "weather", description: "what's the weather" },
    { name: "code-runner" },
  ]),
  listTools: vi.fn().mockResolvedValue([
    { name: "web_search", enabled: true, status: "enabled" },
    { name: "fs_read", enabled: false, status: "disabled" },
  ]),
  setToolEnabled: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../../lib/pico-rest", () => mocks);

beforeEach(() => {
  localStorage.clear();
  localStorage.setItem("clawx.dashboard_token", "T");
  mocks.setToolEnabled.mockClear();
});

describe("ConnectorsPage", () => {
  it("lists skills + tools after token bootstrap", async () => {
    render(
      <MemoryRouter><ClawProvider><ConnectorsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });

    expect(await screen.findByText("weather")).toBeInTheDocument();
    expect(screen.getByText("code-runner")).toBeInTheDocument();
    expect(screen.getByText("web_search")).toBeInTheDocument();
    expect(screen.getByText("fs_read")).toBeInTheDocument();
  });

  it("toggling a tool calls setToolEnabled with token", async () => {
    render(
      <MemoryRouter><ClawProvider><ConnectorsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });

    const toggle = await screen.findByRole("checkbox", { name: /web_search/i });
    expect(toggle).toBeChecked();
    await act(async () => {
      fireEvent.click(toggle);
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(mocks.setToolEnabled).toHaveBeenCalledWith("web_search", false, "T");
  });

  it("with no token, shows the link to Settings", async () => {
    localStorage.clear();
    render(
      <MemoryRouter><ClawProvider><ConnectorsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(screen.getByText(/no dashboard token/i)).toBeInTheDocument();
  });
});
