import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Button } from "./Button";
import { EmptyState } from "./EmptyState";

describe("EmptyState", () => {
  it("renders the title as a heading in th-empty", () => {
    const { container } = render(<EmptyState title="no agents yet" />);
    expect(container.firstElementChild).toHaveClass("th-empty");
    const title = screen.getByRole("heading", { name: "no agents yet" });
    expect(title).toHaveClass("th-empty-title");
  });

  it("renders the body and the action", () => {
    render(
      <EmptyState
        title="add your first repository"
        action={<Button variant="primary">add repository…</Button>}
      >
        townhall works in git worktrees of your repos
      </EmptyState>,
    );
    expect(screen.getByText("townhall works in git worktrees of your repos").tagName).toBe("P");
    expect(screen.getByRole("button", { name: "add repository…" })).toHaveClass("th-btn--primary");
  });

  it("shows the pixel logo at 108px above the title", () => {
    const { container } = render(<EmptyState logo title="add your first repository" />);
    const logo = container.querySelector(".th-empty > svg.th-logo");
    expect(logo).toHaveAttribute("width", "108");
    expect(logo).toHaveAttribute("aria-hidden", "true");
    expect(container.firstElementChild?.firstElementChild).toBe(logo);
  });

  it("leaves out the body and the logo when not given", () => {
    const { container } = render(<EmptyState title="nothing here" />);
    expect(container.querySelector("p")).toBeNull();
    expect(container.querySelector(".th-logo")).toBeNull();
  });
});
