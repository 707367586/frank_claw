import { describe, expect, it, beforeEach } from "vitest";
import {
  appendDelta,
  beginStream,
  clearStream,
  failStream,
  finishStream,
  getStreamState,
  subscribe,
  __resetStreamStore,
} from "../chat-stream-store";

beforeEach(() => {
  __resetStreamStore();
});

describe("chat-stream-store", () => {
  it("returns empty state for an unknown conv", () => {
    expect(getStreamState("never-started")).toEqual({
      content: "",
      isStreaming: false,
      error: null,
    });
  });

  it("accumulates deltas across a single stream", () => {
    beginStream("c1");
    appendDelta("c1", "Hello ");
    appendDelta("c1", "world");
    expect(getStreamState("c1")).toEqual({
      content: "Hello world",
      isStreaming: true,
      error: null,
    });
  });

  it("survives unrelated consumer churn — state persists until cleared", () => {
    beginStream("c1");
    appendDelta("c1", "partial");
    // Simulate ChatPage unmounting: we don't call clear or finish.
    expect(getStreamState("c1").content).toBe("partial");
    expect(getStreamState("c1").isStreaming).toBe(true);
  });

  it("finishStream flips isStreaming but keeps content", () => {
    beginStream("c1");
    appendDelta("c1", "done text");
    finishStream("c1");
    expect(getStreamState("c1")).toEqual({
      content: "done text",
      isStreaming: false,
      error: null,
    });
  });

  it("failStream surfaces error", () => {
    beginStream("c1");
    appendDelta("c1", "partial");
    failStream("c1", "boom");
    expect(getStreamState("c1")).toEqual({
      content: "partial",
      isStreaming: false,
      error: "boom",
    });
  });

  it("clearStream removes the slot entirely", () => {
    beginStream("c1");
    appendDelta("c1", "x");
    clearStream("c1");
    expect(getStreamState("c1")).toEqual({
      content: "",
      isStreaming: false,
      error: null,
    });
  });

  it("notifies subscribers on every mutation", () => {
    let count = 0;
    const unsub = subscribe(() => {
      count += 1;
    });
    beginStream("c1");
    appendDelta("c1", "a");
    appendDelta("c1", "b");
    finishStream("c1");
    unsub();
    // After unsubscribe, further writes should not increment count.
    appendDelta("c1", "c");
    expect(count).toBe(4);
  });

  it("keeps independent state per conv", () => {
    beginStream("c1");
    appendDelta("c1", "A1");
    beginStream("c2");
    appendDelta("c2", "B1");
    expect(getStreamState("c1").content).toBe("A1");
    expect(getStreamState("c2").content).toBe("B1");
  });
});
