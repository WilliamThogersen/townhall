import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ControlHead } from "./ControlHead";
import { ControlStats } from "./ControlStats";

describe("ControlHead", () => {
  it("shows the title, the agent count and a folded new agent button", async () => {
    const onToggleForm = vi.fn();
    const { container } = render(<ControlHead count={2} formOpen={false} onToggleForm={onToggleForm} />);
    expect(container.firstChild).toHaveClass("th-page-head");
    expect(container.firstChild).toHaveAttribute("data-tauri-drag-region", "deep");
    expect(screen.getByRole("heading", { level: 1, name: "control center" })).toBeInTheDocument();
    expect(screen.getByText("2 agents")).toHaveClass("th-control-cap");
    const button = screen.getByRole("button", { name: "new agent…" });
    expect(button).toHaveClass("th-btn", "th-btn--ghost", "th-btn--sm");
    expect(button).toHaveAttribute("aria-expanded", "false");
    await userEvent.click(button);
    expect(onToggleForm).toHaveBeenCalledOnce();
  });

  it("says agent for one and marks the button expanded while the form is open", () => {
    render(<ControlHead count={1} formOpen onToggleForm={vi.fn()} />);
    expect(screen.getByText("1 agent")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "new agent…" })).toHaveAttribute("aria-expanded", "true");
  });

  it("leaves the count out while it is unknown", () => {
    const { container } = render(<ControlHead count={null} formOpen={false} onToggleForm={vi.fn()} />);
    expect(container.querySelector(".th-control-cap")).toBeNull();
  });
});

describe("ControlStats", () => {
  it("renders one trace stat per entry with its label and value", () => {
    const { container } = render(
      <ControlStats
        stats={[
          { label: "agents", value: 3 },
          { label: "running", value: 2 },
          { label: "ended", value: 1 },
        ]}
      />,
    );
    expect(container.firstChild).toHaveClass("th-trace-stats");
    const stats = [...container.querySelectorAll(".th-trace-stat")];
    expect(stats.map((s) => [s.querySelector("span")?.textContent, s.querySelector("b")?.textContent])).toEqual([
      ["agents", "3"],
      ["running", "2"],
      ["ended", "1"],
    ]);
  });
});
