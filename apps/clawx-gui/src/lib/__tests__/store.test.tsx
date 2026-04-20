import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { ClawProvider, useClaw, type ClawContextValue } from "../store";

vi.mock("../pico-rest", () => ({
  fetchPicoInfo: vi.fn().mockResolvedValue({
    configured: true,
    enabled: true,
    ws_url: "ws://localhost:18800/pico/ws",
  }),
}));

beforeEach(() => {
  localStorage.clear();
});

describe("ClawProvider / useClaw", () => {
  it("with no stored token, exposes null token + disabled until set", async () => {
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => {});
    const v: ClawContextValue = result.current;
    expect(v.token).toBeNull();
    expect(v.enabled).toBe(false);
    expect(typeof v.setToken).toBe("function");
    expect(typeof v.startNewSession).toBe("function");
    expect(typeof v.sendUserMessage).toBe("function");
    expect(v.chat.messages).toEqual([]);
  });

  it("with stored token, fetches PicoInfo and exposes ws_url + enabled", async () => {
    localStorage.setItem("clawx.dashboard_token", "STORED");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => {});
    const v: ClawContextValue = result.current;
    expect(v.token).toBe("STORED");
    expect(v.wsUrl).toBe("ws://localhost:18800/pico/ws");
    expect(v.enabled).toBe(true);
    const { fetchPicoInfo } = await import("../pico-rest");
    expect(fetchPicoInfo).toHaveBeenCalledWith("STORED");
  });

  it("setToken persists to localStorage and triggers info fetch", async () => {
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => {});
    await act(async () => {
      result.current.setToken("NEWTOKEN");
    });
    expect(localStorage.getItem("clawx.dashboard_token")).toBe("NEWTOKEN");
    expect(result.current.token).toBe("NEWTOKEN");
  });
});
