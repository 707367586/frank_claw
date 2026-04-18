import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

describe("api base url", () => {
  const ORIGINAL = import.meta.env.VITE_API_URL;
  beforeEach(() => {
    vi.resetModules();
    // Note: assigning `undefined` would be coerced to the string "undefined"
    // by Vite's env proxy, so we `delete` the key instead to genuinely trigger
    // the `??` fallback.
    // @ts-expect-error overwrite for test
    delete import.meta.env.VITE_API_URL;
  });
  afterEach(() => {
    // @ts-expect-error restore
    import.meta.env.VITE_API_URL = ORIGINAL;
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
