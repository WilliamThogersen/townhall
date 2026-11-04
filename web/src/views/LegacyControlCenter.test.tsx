import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AgentsProvider } from "../agents/AgentsProvider";
import * as client from "../api/client";
import type { Agent } from "../api/types";
import { LegacyControlCenter } from "./LegacyControlCenter";

vi.mock("../agents/AgentCard", () => ({
  AgentCard: ({ agent, open, onToggle, onStopped }: {
    agent: Agent; open: boolean; onToggle: (n: string) => void; onStopped: () => void;
  }) => (
    <li>
      <button type="button" onClick={() => onToggle(agent.name)}>
        {agent.name} {open ? "open" : "closed"}
      </button>
      <button type="button" onClick={onStopped}>stopped {agent.name}</button>
    </li>
  ),
}));

vi.mock("../agents/CreateAgentForm", () => ({
  CreateAgentForm: ({ onCreated }: { onCreated: (n: string) => void }) => (
    <button type="button" onClick={() => onCreated("b")}>create b</button>
  ),
}));

const mk = (name: string): Agent => ({
  name, kind: "shell", branch: `townhall/${name}`, worktree: `/w/${name}`, alive: true,
});

afterEach(() => vi.restoreAllMocks());

function show() {
  return render(
    <AgentsProvider intervalMs={60_000}>
      <LegacyControlCenter />
    </AgentsProvider>,
  );
}

describe("LegacyControlCenter", () => {
  it("keeps today's header and empty state inside .app", async () => {
    vi.spyOn(client, "listAgents").mockResolvedValue([]);
    const { container } = show();
    expect(container.querySelector(".th-page-body > .app")).not.toBeNull();
    expect(screen.getByRole("heading", { name: "townhall" })).toBeInTheDocument();
    expect(screen.getByText("Loading agents...")).toBeInTheDocument();
    expect(await screen.findByText("No agents yet. Create one above.")).toBeInTheDocument();
    expect(screen.getByText("0 agents")).toBeInTheDocument();
  });

  it("toggles cards and opens a created agent", async () => {
    const list = vi.spyOn(client, "listAgents").mockResolvedValue([mk("a")]);
    show();
    fireEvent.click(await screen.findByRole("button", { name: "a closed" }));
    expect(screen.getByRole("button", { name: "a open" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "a open" }));
    expect(screen.getByRole("button", { name: "a closed" })).toBeInTheDocument();
    list.mockResolvedValue([mk("a"), mk("b")]);
    fireEvent.click(screen.getByRole("button", { name: "create b" }));
    expect(await screen.findByRole("button", { name: "b open" })).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "stopped a" }));
    await waitFor(() => expect(list).toHaveBeenCalledTimes(3));
  });
});
