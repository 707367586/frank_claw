import { describe, expect, it, vi, beforeEach } from "vitest";

describe("connectSSE event routing", () => {
  beforeEach(() => {
    vi.resetModules();
    // drop env override so BASE_URL defaults apply
    delete import.meta.env.VITE_API_URL;
  });

  function makeResponseStream(chunks: string[]): Response {
    const stream = new ReadableStream({
      start(controller) {
        for (const c of chunks) controller.enqueue(new TextEncoder().encode(c));
        controller.close();
      },
    });
    return new Response(stream, {
      status: 200,
      headers: { "content-type": "text/event-stream" },
    });
  }

  it("routes `event: error` SSE frames to onError, not onMessage", async () => {
    const { connectSSE } = await import("../api");
    const onMessage = vi.fn();
    const onDone = vi.fn();
    const onError = vi.fn();

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        makeResponseStream([
          "event: error\n",
          'data: {"error":"boom"}\n\n',
          "event: done\n",
          'data: {"status":"complete"}\n\n',
        ]),
      ),
    );

    connectSSE("/x", onMessage, onDone, onError);

    // give the async loop time to drain
    await new Promise((r) => setTimeout(r, 20));

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ message: expect.stringContaining("boom") }));
    expect(onMessage).not.toHaveBeenCalledWith('{"error":"boom"}');
    expect(onDone).toHaveBeenCalled();
  });

  it("routes `event: delta` data frames through onMessage as before", async () => {
    const { connectSSE } = await import("../api");
    const onMessage = vi.fn();
    const onDone = vi.fn();

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        makeResponseStream([
          "event: delta\n",
          'data: {"delta":"Hi"}\n\n',
          "event: done\n",
          'data: {"status":"complete"}\n\n',
        ]),
      ),
    );

    connectSSE("/x", onMessage, onDone);
    await new Promise((r) => setTimeout(r, 20));

    expect(onMessage).toHaveBeenCalledWith('{"delta":"Hi"}');
    expect(onDone).toHaveBeenCalled();
  });

  it("tolerates plain `data:` frames with no event prefix (legacy format)", async () => {
    const { connectSSE } = await import("../api");
    const onMessage = vi.fn();
    const onDone = vi.fn();

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        makeResponseStream([
          'data: {"delta":"x"}\n\n',
        ]),
      ),
    );

    connectSSE("/x", onMessage, onDone);
    await new Promise((r) => setTimeout(r, 20));

    expect(onMessage).toHaveBeenCalledWith('{"delta":"x"}');
  });

  it("surfaces real-world provider-not-registered backend error to onError", async () => {
    const { connectSSE } = await import("../api");
    const onMessage = vi.fn();
    const onDone = vi.fn();
    const onError = vi.fn();

    const realPayload =
      "LLM provider error: no provider registered for key 'zhipu' (model: glm-5.1)";

    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        makeResponseStream([
          "event: error\n",
          `data: ${JSON.stringify({ error: realPayload })}\n\n`,
          "event: done\n",
          'data: {"status":"complete"}\n\n',
        ]),
      ),
    );

    connectSSE("/x", onMessage, onDone, onError);
    await new Promise((r) => setTimeout(r, 20));

    expect(onError).toHaveBeenCalledTimes(1);
    expect(onError.mock.calls[0][0]).toBeInstanceOf(Error);
    expect((onError.mock.calls[0][0] as Error).message).toBe(realPayload);
    expect(onMessage).not.toHaveBeenCalled();
  });
});
