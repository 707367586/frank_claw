import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { MemoryRouter } from "react-router-dom";
import AgentSidebar from "../AgentSidebar";
import { AgentProvider } from "../../lib/store";

vi.mock("../../lib/api", () => ({
  listAgents: vi.fn().mockResolvedValue([
    { id: "a1", name: "编程助手", role: "Developer", system_prompt: "",
      model_id: "m1", status: "idle", created_at: "", updated_at: "" },
    { id: "a2", name: "研究助手", role: "Researcher", system_prompt: "",
      model_id: "m1", status: "working", created_at: "", updated_at: "" },
  ]),
}));

describe("AgentSidebar", () => {
  it("renders plain status labels without fake counts", async () => {
    render(
      <MemoryRouter>
        <AgentProvider><AgentSidebar /></AgentProvider>
      </MemoryRouter>,
    );
    await waitFor(() => expect(screen.getByText("编程助手")).toBeInTheDocument());
    expect(screen.getByText("Idle")).toBeInTheDocument();
    expect(screen.getByText("Running")).toBeInTheDocument();
    expect(screen.queryByText(/pending/i)).toBeNull();
    expect(screen.queryByText(/2 pending/i)).toBeNull();
  });
});
