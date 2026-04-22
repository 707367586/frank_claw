import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  fetchHermesInfo,
  listSessions,
  getSession,
  deleteSession,
  listSkills,
  listTools,
  setToolEnabled,
  HermesApiError,
} from "../hermes-rest";

const fetchMock = vi.fn();

beforeEach(() => {
  fetchMock.mockReset();
  vi.stubGlobal("fetch", fetchMock);
});

function ok(body: unknown) {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "content-type": "application/json" },
  });
}

describe("hermes-rest", () => {
  it("fetchHermesInfo returns connection info (with token from arg)", async () => {
    fetchMock.mockResolvedValue(ok({ configured: true, ws_url: "ws://x/hermes/ws", enabled: true, provider: null, missing_env_var: null }));
    const info = await fetchHermesInfo("T");
    expect(info.configured).toBe(true);
    expect(info.ws_url).toBe("ws://x/hermes/ws");
    expect(info.enabled).toBe(true);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/hermes/info",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer T" }),
      }),
    );
  });

  it("listSessions passes offset/limit + auth header", async () => {
    fetchMock.mockResolvedValue(ok([]));
    await listSessions({ offset: 10, limit: 20, token: "T" });
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/sessions?offset=10&limit=20",
      expect.objectContaining({
        headers: expect.objectContaining({ Authorization: "Bearer T" }),
      }),
    );
  });

  it("getSession resolves a single session", async () => {
    fetchMock.mockResolvedValue(ok({ id: "s1", messages: [] }));
    const s = await getSession("s1", "T");
    expect(s.id).toBe("s1");
  });

  it("deleteSession sends DELETE", async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));
    await deleteSession("s1", "T");
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/sessions/s1",
      expect.objectContaining({ method: "DELETE" }),
    );
  });

  it("listSkills unwraps {skills: [...]} envelope", async () => {
    fetchMock.mockResolvedValue(ok({ skills: [{ name: "weather" }] }));
    const skills = await listSkills("T");
    expect(skills).toEqual([{ name: "weather" }]);
  });

  it("listTools unwraps {tools: [...]} and derives enabled from status", async () => {
    fetchMock.mockResolvedValue(
      ok({
        tools: [
          { name: "web_search", status: "enabled", category: "web", config_key: "web" },
          { name: "fs_read", status: "disabled" },
          { name: "experimental", status: "blocked", reason_code: "no_hardware" },
        ],
      }),
    );
    const tools = await listTools("T");
    expect(tools).toHaveLength(3);
    expect(tools[0]).toMatchObject({ name: "web_search", status: "enabled", enabled: true });
    expect(tools[1]).toMatchObject({ name: "fs_read", status: "disabled", enabled: false });
    expect(tools[2]).toMatchObject({
      name: "experimental",
      status: "blocked",
      enabled: false,
      reason_code: "no_hardware",
    });
  });

  it("listTools returns [] when wrapper is empty or missing", async () => {
    fetchMock.mockResolvedValue(ok({}));
    const tools = await listTools("T");
    expect(tools).toEqual([]);
  });

  it("setToolEnabled sends PUT with body", async () => {
    fetchMock.mockResolvedValue(new Response(null, { status: 204 }));
    await setToolEnabled("web_search", false, "T");
    expect(fetchMock).toHaveBeenLastCalledWith(
      "/api/tools/web_search/state",
      expect.objectContaining({
        method: "PUT",
        body: JSON.stringify({ enabled: false }),
      }),
    );
  });

  it("throws HermesApiError on non-2xx", async () => {
    fetchMock.mockResolvedValue(new Response(JSON.stringify({ message: "nope" }), { status: 401 }));
    await expect(fetchHermesInfo("T")).rejects.toBeInstanceOf(HermesApiError);
  });
});
