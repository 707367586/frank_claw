import { describe, it, expect, expectTypeOf } from "vitest";
import {
  isServerMessage,
  type PicoMessage,
  type ServerMessageType,
} from "../pico-types";

describe("pico-types", () => {
  it("recognises every server-to-client message type", () => {
    const types: ServerMessageType[] = [
      "message.create",
      "message.update",
      "media.create",
      "typing.start",
      "typing.stop",
      "error",
      "pong",
    ];
    for (const t of types) {
      const msg: PicoMessage = { type: t, payload: {} };
      expect(isServerMessage(msg)).toBe(true);
    }
  });

  it("rejects unknown server types", () => {
    expect(isServerMessage({ type: "wat", payload: {} } as unknown as PicoMessage)).toBe(false);
  });

  it("PicoMessage envelope shape", () => {
    expectTypeOf<PicoMessage>().toHaveProperty("type");
    expectTypeOf<PicoMessage>().toHaveProperty("payload");
  });
});
