import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import CreateAgentModal from "../CreateAgentModal";

const mockCreate = vi.fn().mockResolvedValue({ id: "new" });

vi.mock("../../lib/store", () => ({
  useClaw: () => ({
    toolsets: [
      { name: "web", description: "web tools", tools: [] },
      { name: "file", description: "file tools", tools: [] },
    ],
    createAgent: mockCreate,
  }),
}));

beforeEach(() => mockCreate.mockClear());

describe("CreateAgentModal", () => {
  it("required fields gate submission", async () => {
    const onClose = vi.fn();
    render(<CreateAgentModal open onClose={onClose} />);
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    expect(mockCreate).not.toHaveBeenCalled();
    expect(screen.getByText(/请输入名称/)).toBeInTheDocument();
    expect(screen.getByText(/请输入 System Prompt/)).toBeInTheDocument();
  });

  it("submits payload with defaults: model=null, all toolsets enabled", async () => {
    const onClose = vi.fn();
    render(<CreateAgentModal open onClose={onClose} />);
    fireEvent.change(screen.getByLabelText(/名称/), { target: { value: "X" } });
    fireEvent.change(screen.getByLabelText(/System Prompt/), { target: { value: "be helpful" } });
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    const payload = mockCreate.mock.calls[0][0];
    expect(payload.name).toBe("X");
    expect(payload.system_prompt).toBe("be helpful");
    expect(payload.model).toBeNull();
    expect(payload.enabled_toolsets).toEqual(["web", "file"]);
    expect(onClose).toHaveBeenCalled();
  });

  it("custom model — picking a preset sets the value", async () => {
    render(<CreateAgentModal open onClose={() => {}} />);
    fireEvent.change(screen.getByLabelText(/名称/), { target: { value: "X" } });
    fireEvent.change(screen.getByLabelText(/System Prompt/), { target: { value: "p" } });
    fireEvent.click(screen.getByRole("button", { name: /自定义模型/ }));
    fireEvent.change(screen.getByLabelText(/选择模型/), { target: { value: "Sonnet 4.6" } });
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    expect(mockCreate.mock.calls[0][0].model).toBe("Sonnet 4.6");
  });

  it("toolsets — 全不选 then submit yields []", async () => {
    render(<CreateAgentModal open onClose={() => {}} />);
    fireEvent.change(screen.getByLabelText(/名称/), { target: { value: "X" } });
    fireEvent.change(screen.getByLabelText(/System Prompt/), { target: { value: "p" } });
    fireEvent.click(screen.getByRole("button", { name: /高级/ }));
    fireEvent.click(screen.getByRole("button", { name: /全不选/ }));
    fireEvent.click(screen.getByRole("button", { name: /创建/ }));
    await waitFor(() => expect(mockCreate).toHaveBeenCalledTimes(1));
    expect(mockCreate.mock.calls[0][0].enabled_toolsets).toEqual([]);
  });
});
