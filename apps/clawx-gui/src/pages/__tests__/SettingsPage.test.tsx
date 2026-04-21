import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, act, fireEvent } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import SettingsPage from "../SettingsPage";
import { ClawProvider } from "../../lib/store";

const fetchHermesInfo = vi.hoisted(() =>
  vi.fn().mockResolvedValue({
    configured: true,
    enabled: true,
    ws_url: "ws://localhost:18800/hermes/ws",
  }),
);
vi.mock("../../lib/hermes-rest", () => ({ fetchHermesInfo }));

beforeEach(() => {
  localStorage.clear();
  fetchHermesInfo.mockClear();
});

describe("SettingsPage", () => {
  it("with no token, shows the paste-token form", async () => {
    render(
      <MemoryRouter><ClawProvider><SettingsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(screen.getByLabelText(/dashboard token/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /save/i })).toBeInTheDocument();
  });

  it("pasting a token persists it and refreshes info", async () => {
    render(
      <MemoryRouter><ClawProvider><SettingsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    const input = screen.getByLabelText(/dashboard token/i);
    await act(async () => {
      fireEvent.change(input, { target: { value: "PASTED" } });
      fireEvent.click(screen.getByRole("button", { name: /save/i }));
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(localStorage.getItem("clawx.dashboard_token")).toBe("PASTED");
    expect(fetchHermesInfo).toHaveBeenCalledWith("PASTED");
  });

  it("with a token, shows ws_url + enabled status + Refresh + Clear", async () => {
    localStorage.setItem("clawx.dashboard_token", "EXISTING");
    render(
      <MemoryRouter><ClawProvider><SettingsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(screen.getByText(/ws:\/\/localhost:18800/)).toBeInTheDocument();
    expect(screen.getByText(/enabled/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /refresh/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /clear/i })).toBeInTheDocument();
  });

  it("Refresh re-fetches info", async () => {
    localStorage.setItem("clawx.dashboard_token", "EXISTING");
    render(
      <MemoryRouter><ClawProvider><SettingsPage /></ClawProvider></MemoryRouter>,
    );
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    fetchHermesInfo.mockClear();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: /refresh/i }));
      await new Promise((r) => setTimeout(r, 0));
    });
    expect(fetchHermesInfo).toHaveBeenCalledWith("EXISTING");
  });
});
