import { describe, it, expect, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { ClawProvider, useClaw, type ClawContextValue } from "../store";

vi.mock("../agents-rest", () => ({
  listAgents: vi.fn().mockResolvedValue([
    {
      id: "a1", name: "n", description: "", color: "#5749F4", icon: "Bot",
      system_prompt: "p", model: null, enabled_toolsets: [],
      workspace_dir: "/tmp/a1", current_session_id: "sid-1", created_at: 1,
    },
  ]),
  listToolsets: vi.fn().mockResolvedValue([
    { name: "web", description: "", tools: [] },
  ]),
  createAgent: vi.fn(),
  deleteAgent: vi.fn().mockResolvedValue(undefined),
  rotateAgentSession: vi.fn().mockResolvedValue({ session_id: "sid-2" }),
}));

vi.mock("../hermes-rest", async () => {
  const actual = await vi.importActual<typeof import("../hermes-rest")>("../hermes-rest");
  return {
    ...actual,
    fetchHermesInfo: vi.fn().mockResolvedValue({
      configured: true, enabled: true, ws_url: "ws://x", provider: null, missing_env_var: null,
    }),
    getSession: vi.fn().mockResolvedValue({
      id: "sid-1", title: "", preview: "", message_count: 1,
      created: 0, updated: 0, summary: "",
      messages: [{ role: "user", content: "hi" }],
    }),
  };
});

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
    expect(typeof v.selectAgent).toBe("function");
    expect(typeof v.newConversation).toBe("function");
    expect(typeof v.sendUserMessage).toBe("function");
    expect(v.chat.messages).toEqual([]);
  });

  it("with stored token, fetches HermesInfo and exposes ws_url + enabled", async () => {
    localStorage.setItem("clawx.dashboard_token", "STORED");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => {});
    const v: ClawContextValue = result.current;
    expect(v.token).toBe("STORED");
    expect(v.wsUrl).toBe("ws://x");
    expect(v.enabled).toBe(true);
    const { fetchHermesInfo } = await import("../hermes-rest");
    expect(fetchHermesInfo).toHaveBeenCalledWith("STORED");
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

describe("ClawProvider — agents", () => {
  it("loads agents + toolsets after token bootstraps", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(result.current.agents.map((a) => a.id)).toEqual(["a1"]);
    expect(result.current.toolsets[0].name).toBe("web");
  });

  it("auto-selects first agent and hydrates history", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    expect(result.current.activeAgentId).toBe("a1");
    expect(result.current.chat.messages.map((m) => m.content)).toEqual(["hi"]);
  });

  it("newConversation rotates session_id and clears messages", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    await act(async () => {
      await result.current.newConversation();
    });
    const a = result.current.agents.find((x) => x.id === "a1")!;
    expect(a.current_session_id).toBe("sid-2");
    expect(result.current.chat.messages).toEqual([]);
  });

  it("deleteAgent splices and falls back to first remaining", async () => {
    localStorage.setItem("clawx.dashboard_token", "T");
    const { result } = renderHook(() => useClaw(), { wrapper: ClawProvider });
    await act(async () => { await new Promise((r) => setTimeout(r, 0)); });
    await act(async () => { await result.current.deleteAgent("a1"); });
    expect(result.current.agents).toEqual([]);
    expect(result.current.activeAgentId).toBeNull();
  });
});
