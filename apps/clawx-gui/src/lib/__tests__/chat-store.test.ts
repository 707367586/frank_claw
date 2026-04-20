import { describe, it, expect, beforeEach } from "vitest";
import { ChatStore } from "../chat-store";

describe("ChatStore", () => {
  let s: ChatStore;
  beforeEach(() => { s = new ChatStore(); });

  it("addUser optimistically appends user message", () => {
    const id = s.addUser("hi");
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]).toMatchObject({ id, role: "user", content: "hi" });
  });

  it("applyServer message.create appends assistant message", () => {
    s.applyServer({ type: "message.create", payload: { message_id: "m1", content: "hello" } });
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]).toMatchObject({
      id: "m1", role: "assistant", content: "hello", thought: false,
    });
  });

  it("applyServer message.update merges by message_id", () => {
    s.applyServer({ type: "message.create", payload: { message_id: "m1", content: "hel" } });
    s.applyServer({ type: "message.update", payload: { message_id: "m1", content: "hello world" } });
    expect(s.messages).toHaveLength(1);
    expect(s.messages[0]!.content).toBe("hello world");
  });

  it("thought:true messages tagged as thought", () => {
    s.applyServer({ type: "message.create", payload: { message_id: "t1", content: "thinking…", thought: true } });
    expect(s.messages[0]!.thought).toBe(true);
  });

  it("typing.start / typing.stop toggle typing flag", () => {
    s.applyServer({ type: "typing.start", payload: {} });
    expect(s.typing).toBe(true);
    s.applyServer({ type: "typing.stop", payload: {} });
    expect(s.typing).toBe(false);
  });

  it("error with request_id rolls back optimistic user message", () => {
    const id = s.addUser("oops", "REQ1");
    expect(s.messages).toHaveLength(1);
    s.applyServer({ type: "error", payload: { code: "RATE_LIMIT", message: "slow down", request_id: "REQ1" } });
    expect(s.messages.find((m) => m.id === id)).toBeUndefined();
    expect(s.lastError?.code).toBe("RATE_LIMIT");
  });

  it("subscribers fire on every state change", () => {
    let calls = 0;
    s.subscribe(() => calls++);
    s.addUser("a");
    s.applyServer({ type: "message.create", payload: { message_id: "m", content: "b" } });
    expect(calls).toBeGreaterThanOrEqual(2);
  });
});
