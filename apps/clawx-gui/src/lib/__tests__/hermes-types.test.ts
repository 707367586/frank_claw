import { describe, it, expect, expectTypeOf } from "vitest";
import {
  isServerMessage,
  type HermesMessage,
  type ServerMessageType,
} from "../hermes-types";

describe("hermes-types", () => {
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
      const msg: HermesMessage = { type: t, payload: {} };
      expect(isServerMessage(msg)).toBe(true);
    }
  });

  it("rejects unknown server types", () => {
    expect(isServerMessage({ type: "wat", payload: {} } as unknown as HermesMessage)).toBe(false);
  });

  it("HermesMessage envelope shape", () => {
    expectTypeOf<HermesMessage>().toHaveProperty("type");
    expectTypeOf<HermesMessage>().toHaveProperty("payload");
  });
});
