import { describe, expect, it, vi, beforeEach } from "vitest";

describe("api base url", () => {
  beforeEach(() => {
    vi.resetModules();
    // Remove .env.local override so the `??` fallback runs.
    // Can't assign undefined: Vite's env Proxy coerces to the string "undefined".
    // Can't rely on afterEach restore either — but vitest's unstubEnvs:true handles it.
    delete import.meta.env.VITE_API_URL;
  });

  it("defaults to 127.0.0.1 to avoid macOS IPv6 localhost resolution", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response("[]", { status: 200, headers: { "content-type": "application/json" } }),
    );
    vi.stubGlobal("fetch", fetchMock);
    const { listAgents } = await import("../api");
    await listAgents();
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:9090/agents",
      expect.any(Object),
    );
  });

  it("updateModel PUTs to /models/:id with partial payload", async () => {
    const fetchMock = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ id: "p1" }), {
        status: 200,
        headers: { "content-type": "application/json" },
      }),
    );
    vi.stubGlobal("fetch", fetchMock);
    const { updateModel } = await import("../api");
    await updateModel("p1", { parameters: { api_key: "k" } });
    expect(fetchMock).toHaveBeenCalledWith(
      "http://127.0.0.1:9090/models/p1",
      expect.objectContaining({ method: "PUT" }),
    );
    const call = fetchMock.mock.calls[0];
    expect(JSON.parse(call[1].body as string)).toEqual({
      parameters: { api_key: "k" },
    });
  });
});

describe("env isolation", () => {
  it("vitest auto-restores fetch stub between tests", () => {
    // If unstubGlobals: true is honored, global.fetch is back to the jsdom default here.
    expect(typeof globalThis.fetch).toBe("function");
    // And it's not the previous describe's mock — the mock had .mock metadata:
    expect((globalThis.fetch as any).mock).toBeUndefined();
  });
});
