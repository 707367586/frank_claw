import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { MemoryRouter, Routes, Route, useLocation } from "react-router-dom";
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
    expect(screen.getByText("空闲")).toBeInTheDocument();
    expect(screen.getByText("运行中")).toBeInTheDocument();
    expect(screen.queryByText(/pending/i)).toBeNull();
    expect(screen.queryByText(/2 pending/i)).toBeNull();
  });

  it("drops the active conv query param when switching to a different agent", async () => {
    function UrlProbe() {
      const { search } = useLocation();
      return <span data-testid="probe">{search}</span>;
    }

    render(
      <MemoryRouter initialEntries={["/?conv=c1&agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route
              path="/"
              element={
                <>
                  <AgentSidebar />
                  <UrlProbe />
                </>
              }
            />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );

    // wait for agents to load, then click the second agent
    await waitFor(() => expect(screen.getByText("研究助手")).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: /研究助手/ }));

    await waitFor(() => {
      const search = screen.getByTestId("probe").textContent ?? "";
      expect(search).toContain("agent=a2");
      expect(search).not.toContain("conv=");
    });
  });

  it("keeps conv when clicking the already-selected agent", async () => {
    function UrlProbe() {
      const { search } = useLocation();
      return <span data-testid="probe">{search}</span>;
    }

    render(
      <MemoryRouter initialEntries={["/?conv=c1&agent=a1"]}>
        <AgentProvider>
          <Routes>
            <Route
              path="/"
              element={
                <>
                  <AgentSidebar />
                  <UrlProbe />
                </>
              }
            />
          </Routes>
        </AgentProvider>
      </MemoryRouter>,
    );

    await waitFor(() => expect(screen.getByText("编程助手")).toBeInTheDocument());
    await userEvent.click(screen.getByRole("button", { name: /编程助手/ }));

    // Same agent re-selected: conv stays.
    await waitFor(() => {
      const search = screen.getByTestId("probe").textContent ?? "";
      expect(search).toContain("agent=a1");
      expect(search).toContain("conv=c1");
    });
  });
});
