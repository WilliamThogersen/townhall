import { render, renderHook, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import type { AgentInfo } from "../api/types";
import * as notify from "../desktop/notify";
import { AgentsProvider, useAgentsContext } from "./AgentsProvider";

const mk = (name: string, alive: boolean): AgentInfo => ({
  name, kind: "shell", branch: `townhall/${name}`, worktree: `/w/${name}`, alive,
  repo: "demo", state: { kind: alive ? "working" : "ended" },
});

afterEach(() => vi.restoreAllMocks());

function Names({ label }: { label: string }) {
  const { agents, loading } = useAgentsContext();
  return <p>{loading ? `${label} loading` : `${label}: ${agents.map((a) => a.name).join(",")}`}</p>;
}

describe("AgentsProvider", () => {
  it("polls once for every consumer", async () => {
    const spy = vi.spyOn(client, "listAgents").mockResolvedValue([mk("a", true)]);
    render(
      <AgentsProvider intervalMs={60_000}>
        <Names label="one" />
        <Names label="two" />
      </AgentsProvider>,
    );
    expect(await screen.findByText("one: a")).toBeInTheDocument();
    expect(screen.getByText("two: a")).toBeInTheDocument();
    expect(spy).toHaveBeenCalledOnce();
  });

  it("notifies when an agent dies", async () => {
    const died = vi.spyOn(notify, "notifyAgentDied").mockResolvedValue();
    const spy = vi.spyOn(client, "listAgents").mockResolvedValue([mk("a", true)]);
    let refresh: () => Promise<void> = async () => {};
    function Grab() {
      refresh = useAgentsContext().refresh;
      return null;
    }
    render(
      <AgentsProvider intervalMs={60_000}>
        <Grab />
      </AgentsProvider>,
    );
    await waitFor(() => expect(spy).toHaveBeenCalledOnce());
    spy.mockResolvedValue([mk("a", false)]);
    await refresh();
    await waitFor(() => expect(died).toHaveBeenCalledExactlyOnceWith("a"));
  });

  it("throws a clear error outside the provider", () => {
    vi.spyOn(console, "error").mockImplementation(() => {});
    expect(() => renderHook(() => useAgentsContext())).toThrow(
      "useAgentsContext needs an AgentsProvider",
    );
  });
});
