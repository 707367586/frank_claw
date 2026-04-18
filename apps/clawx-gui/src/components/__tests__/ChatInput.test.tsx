import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import ChatInput from "../ChatInput";

describe("ChatInput", () => {
  it("renders the model passed in, not a hardcoded default", () => {
    render(<ChatInput onSend={() => {}} model="glm-4.6" />);
    expect(screen.getByText("glm-4.6")).toBeInTheDocument();
    expect(screen.queryByText("Sonnet 4.6")).toBeNull();
  });

  it("shows `未选择` placeholder when no model supplied", () => {
    render(<ChatInput onSend={() => {}} />);
    expect(screen.getByText("未选择")).toBeInTheDocument();
  });

  it("sends on Enter and clears the input", async () => {
    const onSend = vi.fn();
    render(<ChatInput onSend={onSend} model="glm-4.6" />);
    const field = screen.getByPlaceholderText("输入任何问题...");
    await userEvent.type(field, "你好{enter}");
    expect(onSend).toHaveBeenCalledWith("你好");
    expect(field).toHaveValue("");
  });
});
