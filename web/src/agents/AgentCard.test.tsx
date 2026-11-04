import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Agent } from "../api/types";
import { AgentCard } from "./AgentCard";

vi.mock("./AgentPane", () => ({
  AgentPane: ({ agent, onClose, onStopped }: { agent: Agent; onClose: () => void; onStopped: () => void }) => (
    <article aria-label={`${agent.name} terminal`}>
      <button onClick={onClose}>close</button>
      <button onClick={onStopped}>stopped</button>
    </article>
  ),
}));

const agent: Agent = { name: "u-demo", kind: "shell", branch: "townhall/u-demo", worktree: "/w", alive: true };

describe("AgentCard", () => {
  it("shows the summary and opens on request when closed", async () => {
    const onToggle = vi.fn();
    render(<AgentCard agent={agent} open={false} onToggle={onToggle} onStopped={() => {}} />);
    expect(screen.getByText("u-demo")).toBeInTheDocument();
    expect(screen.queryByRole("article")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    expect(onToggle).toHaveBeenCalledWith("u-demo");
  });

  it("renders the agent pane in a fixed box when open", async () => {
    const onToggle = vi.fn();
    const onStopped = vi.fn();
    const { container } = render(
      <AgentCard agent={agent} open onToggle={onToggle} onStopped={onStopped} />,
    );
    const pane = screen.getByRole("article", { name: "u-demo terminal" });
    expect(pane.parentElement).toBe(container.querySelector(".card.open > .card-pane"));
    await userEvent.click(screen.getByRole("button", { name: "close" }));
    expect(onToggle).toHaveBeenCalledWith("u-demo");
    await userEvent.click(screen.getByRole("button", { name: "stopped" }));
    expect(onStopped).toHaveBeenCalledTimes(1);
  });
});
