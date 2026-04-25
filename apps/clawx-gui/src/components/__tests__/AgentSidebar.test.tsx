import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import AgentSidebar from "../AgentSidebar";

const mockSelect = vi.fn();
const mockDelete = vi.fn();

vi.mock("../../lib/store", () => ({
  useClaw: () => ({
    agents: [
      { id: "a1", name: "编程助手", description: "", color: "#5749F4", icon: "Code2",
        system_prompt: "", model: null, enabled_toolsets: [],
        workspace_dir: "/", current_session_id: "s", created_at: 0 },
      { id: "a2", name: "研究助手", description: "", color: "#3B82F6", icon: "Search",
        system_prompt: "", model: null, enabled_toolsets: [],
        workspace_dir: "/", current_session_id: "s", created_at: 0 },
    ],
    activeAgentId: "a1",
    chat: { typing: false, messages: [] },
    selectAgent: mockSelect,
    deleteAgent: mockDelete,
  }),
}));

describe("AgentSidebar", () => {
  it("renders all agents and marks active", () => {
    render(<AgentSidebar />);
    const a1 = screen.getByRole("button", { name: /编程助手/ });
    const a2 = screen.getByRole("button", { name: /研究助手/ });
    expect(a1.className).toContain("is-active");
    expect(a2.className).not.toContain("is-active");
  });

  it("clicking an agent calls selectAgent", () => {
    render(<AgentSidebar />);
    fireEvent.click(screen.getByRole("button", { name: /研究助手/ }));
    expect(mockSelect).toHaveBeenCalledWith("a2");
  });

  it("delete button confirms then calls deleteAgent", () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(<AgentSidebar />);
    fireEvent.click(screen.getByLabelText(/删除 编程助手/));
    expect(mockDelete).toHaveBeenCalledWith("a1");
  });

  it("delete cancelled — does not call deleteAgent", () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(<AgentSidebar />);
    fireEvent.click(screen.getByLabelText(/删除 研究助手/));
    expect(mockDelete).not.toHaveBeenCalledWith("a2");
  });
});
