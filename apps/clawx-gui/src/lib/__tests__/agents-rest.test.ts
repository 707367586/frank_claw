import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  listAgents,
  createAgent,
  deleteAgent,
  rotateAgentSession,
  listToolsets,
} from "../agents-rest";
import { HermesApiError } from "../hermes-rest";

const mockFetch = vi.fn();

beforeEach(() => {
  vi.stubGlobal("fetch", mockFetch);
});
afterEach(() => {
  mockFetch.mockReset();
  vi.unstubAllGlobals();
});

function ok(body: unknown, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: "ok",
    json: async () => body,
  } as Response;
}

describe("agents-rest", () => {
  it("listAgents calls GET /api/agents with bearer", async () => {
    mockFetch.mockResolvedValueOnce(ok([{ id: "a1" }]));
    const out = await listAgents("T");
    expect(out).toEqual([{ id: "a1" }]);
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents");
    expect((init as RequestInit).headers).toMatchObject({ Authorization: "Bearer T" });
  });

  it("createAgent POSTs JSON body", async () => {
    mockFetch.mockResolvedValueOnce(ok({ id: "new" }, 201));
    const payload = {
      name: "X", description: "", color: "#5749F4", icon: "Bot",
      system_prompt: "p", model: null, enabled_toolsets: ["web"],
    };
    const out = await createAgent(payload, "T");
    expect(out).toEqual({ id: "new" });
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents");
    expect((init as RequestInit).method).toBe("POST");
    expect(JSON.parse((init as RequestInit).body as string)).toEqual(payload);
  });

  it("deleteAgent DELETE /api/agents/:id", async () => {
    mockFetch.mockResolvedValueOnce({ ok: true, status: 204 } as Response);
    await deleteAgent("aid", "T");
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents/aid");
    expect((init as RequestInit).method).toBe("DELETE");
  });

  it("rotateAgentSession POSTs to /sessions and returns body", async () => {
    mockFetch.mockResolvedValueOnce(ok({ session_id: "rot" }));
    const out = await rotateAgentSession("aid", "T");
    expect(out).toEqual({ session_id: "rot" });
    const [url, init] = mockFetch.mock.calls[0];
    expect(url).toBe("/api/agents/aid/sessions");
    expect((init as RequestInit).method).toBe("POST");
  });

  it("listToolsets calls GET /api/toolsets", async () => {
    mockFetch.mockResolvedValueOnce(ok([{ name: "web", description: "", tools: [] }]));
    const out = await listToolsets("T");
    expect(out[0].name).toBe("web");
  });

  it("throws HermesApiError on non-2xx with backend message", async () => {
    mockFetch.mockResolvedValueOnce({
      ok: false,
      status: 404,
      statusText: "Not Found",
      json: async () => ({ message: "agent not found" }),
    } as Response);
    const err = await deleteAgent("nope", "T").catch((e) => e);
    expect(err).toBeInstanceOf(HermesApiError);
    expect(err.status).toBe(404);
    expect(err.message).toBe("agent not found");
  });
});
