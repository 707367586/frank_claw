import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import ChatWelcome from "../ChatWelcome";
import type { Agent } from "../../lib/types";

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

  it("renders fallback title and subtitle when no agent is passed", () => {
    render(<ChatWelcome />);
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent("ClawX");
    expect(screen.getByText("选中一个 Agent 开始对话，或在下方输入问题。")).toBeInTheDocument();
  });

  it("truncates long system prompts with an ellipsis", () => {
    const long = "a".repeat(100);
    render(
      <ChatWelcome agent={{ ...agent, system_prompt: long }} />,
    );
    const subtitle = screen.getByText((text) => text.startsWith("aaaa") && text.endsWith("…"));
    expect(subtitle).toBeInTheDocument();
  });
});
