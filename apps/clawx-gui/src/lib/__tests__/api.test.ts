import { describe, expect, it, vi, beforeEach } from "vitest";

describe("api base url", () => {
  beforeEach(() => {
    vi.resetModules();
    // Remove .env.local override so the `??` fallback runs.
    // Can't assign undefined: Vite's env Proxy coerces to the string "undefined".
    // Can't rely on afterEach restore either — but vitest's unstubEnvs:true handles it.
    // @ts-expect-error delete override
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
});

describe("env isolation", () => {
  it("vitest auto-restores fetch stub between tests", () => {
    // If unstubGlobals: true is honored, global.fetch is back to the jsdom default here.
    expect(typeof globalThis.fetch).toBe("function");
    // And it's not the previous describe's mock — the mock had .mock metadata:
    expect((globalThis.fetch as any).mock).toBeUndefined();
  });
});
