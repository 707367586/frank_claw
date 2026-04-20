import { describe, it, expect, beforeEach, vi } from "vitest";
import {
  fetchPicoInfo,
  listSessions,
  getSession,
  deleteSession,
  listSkills,
  listTools,
  setToolEnabled,
  PicoApiError,
} from "../pico-rest";

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

describe("pico-rest", () => {
  it("fetchPicoInfo returns connection info (with token from arg)", async () => {
    fetchMock.mockResolvedValue(ok({ configured: true, ws_url: "ws://x/pico/ws", enabled: true }));
    const info = await fetchPicoInfo("T");
    expect(info.configured).toBe(true);
    expect(info.ws_url).toBe("ws://x/pico/ws");
    expect(info.enabled).toBe(true);
    expect(fetchMock).toHaveBeenCalledWith(
      "/api/pico/info",
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

  it("listTools returns the array directly", async () => {
    fetchMock.mockResolvedValue(ok([{ name: "web_search", enabled: true }]));
    const tools = await listTools("T");
    expect(tools).toEqual([{ name: "web_search", enabled: true }]);
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

  it("throws PicoApiError on non-2xx", async () => {
    fetchMock.mockResolvedValue(new Response(JSON.stringify({ message: "nope" }), { status: 401 }));
    await expect(fetchPicoInfo("T")).rejects.toBeInstanceOf(PicoApiError);
  });
});
