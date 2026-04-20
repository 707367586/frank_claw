import { describe, it, expect, beforeEach, vi } from "vitest";
import { PicoSocket } from "../pico-socket";
import type { PicoMessage } from "../pico-types";

class FakeWS {
  static instances: FakeWS[] = [];
  url: string;
  protocols?: string | string[];
  readyState = 0;
  onopen?: () => void;
  onclose?: (e: { code: number; reason: string }) => void;
  onerror?: (e: unknown) => void;
  onmessage?: (e: { data: string }) => void;
  sent: string[] = [];

  constructor(url: string, protocols?: string | string[]) {
    this.url = url;
    this.protocols = protocols;
    FakeWS.instances.push(this);
  }
  send(data: string) { this.sent.push(data); }
  close(code = 1000) {
    this.readyState = 3;
    this.onclose?.({ code, reason: "" });
  }
  open() {
    this.readyState = 1;
    this.onopen?.();
  }
  emit(msg: PicoMessage) {
    this.onmessage?.({ data: JSON.stringify(msg) });
  }
}

beforeEach(() => {
  FakeWS.instances = [];
  vi.stubGlobal("WebSocket", FakeWS as unknown as typeof WebSocket);
  // The PicoSocket implementation uses WebSocket.OPEN constant when checking
  // readyState; FakeWS doesn't define statics, so polyfill them.
  (FakeWS as unknown as { OPEN: number }).OPEN = 1;
});

describe("PicoSocket", () => {
  it("connects with token subprotocol and session_id query", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    const ws = FakeWS.instances[0]!;
    expect(ws.url).toBe("ws://h/pico/ws?session_id=S1");
    expect(ws.protocols).toEqual(["token.TKN"]);
  });

  it("dispatches parsed server messages to onMessage", () => {
    const onMsg = vi.fn();
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN", onMessage: onMsg });
    s.connect();
    const ws = FakeWS.instances[0]!;
    ws.open();
    ws.emit({ type: "message.create", payload: { message_id: "m1", content: "hi" } });
    expect(onMsg).toHaveBeenCalledWith(expect.objectContaining({ type: "message.create" }));
  });

  it("send wraps client message into envelope JSON", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    const ws = FakeWS.instances[0]!;
    ws.open();
    s.send({ type: "message.send", payload: { content: "hello" } });
    expect(ws.sent).toHaveLength(1);
    const sent = JSON.parse(ws.sent[0]!);
    expect(sent.type).toBe("message.send");
    expect(sent.session_id).toBe("S1");
    expect(typeof sent.id).toBe("string");
    expect(typeof sent.timestamp).toBe("number");
  });

  it("queues sends until socket open, flushes on open", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    const ws = FakeWS.instances[0]!;
    s.send({ type: "message.send", payload: { content: "queued" } });
    expect(ws.sent).toHaveLength(0);
    ws.open();
    expect(ws.sent).toHaveLength(1);
  });

  it("close stops further reconnects", () => {
    const s = new PicoSocket({ wsBase: "ws://h/pico/ws", sessionId: "S1", token: "TKN" });
    s.connect();
    s.close();
    const ws = FakeWS.instances[0]!;
    ws.close(1006);
    expect(FakeWS.instances).toHaveLength(1);
  });
});
