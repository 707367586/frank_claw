import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import ChatWelcome from "../ChatWelcome";
import type { Agent } from "../../lib/types";

afterEach(() => {
  cleanup();
});

const agent: Agent = {
  id: "a1",
  name: "编程助手",
  role: "Developer",
  system_prompt: "helper",
  model_id: "m1",
  status: "idle",
  created_at: "",
  updated_at: "",
};

describe("ChatWelcome", () => {
  it("shows the selected agent's name, not a hardcoded MaxClaw", () => {
    render(<ChatWelcome agent={agent} />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("编程助手");
    expect(screen.queryByText("MaxClaw")).toBeNull();
  });

  it("forwards chip clicks through onSend", async () => {
    const onSend = vi.fn();
    render(<ChatWelcome agent={agent} onSend={onSend} />);
    await userEvent.click(screen.getByRole("button", { name: "对话" }));
    expect(onSend).toHaveBeenCalledWith("对话");
  });
});
