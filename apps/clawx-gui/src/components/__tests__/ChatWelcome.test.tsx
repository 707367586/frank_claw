import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import ChatWelcome from "../ChatWelcome";

describe("ChatWelcome", () => {
  it("renders with chat-welcome testid", () => {
    render(<ChatWelcome />);
    expect(screen.getByTestId("chat-welcome")).toBeInTheDocument();
  });
});
