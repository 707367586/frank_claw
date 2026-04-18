import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

describe("vitest smoke", () => {
  it("renders plain JSX", () => {
    render(<h1>hello-claw</h1>);
    expect(screen.getByText("hello-claw")).toBeInTheDocument();
  });
});
