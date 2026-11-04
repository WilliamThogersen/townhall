import { describe, expect, it } from "vitest";
import type { Agent } from "../../api/types";
import { controlStats, findAgent, sortAgents } from "./controlModel";

const mk = (name: string, alive: boolean): Agent => ({
  name,
  kind: "shell",
  branch: `townhall/${name}`,
  worktree: `/w/${name}`,
  alive,
});

describe("sortAgents", () => {
  it("puts alive agents first, then sorts by name", () => {
    const agents = [mk("d", false), mk("b", true), mk("a", false), mk("c", true)];
    expect(sortAgents(agents).map((a) => a.name)).toEqual(["b", "c", "a", "d"]);
  });

  it("leaves the input untouched", () => {
    const agents = [mk("b", true), mk("a", true)];
    sortAgents(agents);
    expect(agents.map((a) => a.name)).toEqual(["b", "a"]);
  });
});

describe("controlStats", () => {
  it("counts agents, running and ended", () => {
    expect(controlStats([mk("a", true), mk("b", false), mk("c", true)])).toEqual([
      { label: "agents", value: 3 },
      { label: "running", value: 2 },
      { label: "ended", value: 1 },
    ]);
  });

  it("shows zeros for an empty list", () => {
    expect(controlStats([]).map((s) => s.value)).toEqual([0, 0, 0]);
  });
});

describe("findAgent", () => {
  it("finds an agent by name or gives null", () => {
    const agents = [mk("a", true)];
    expect(findAgent(agents, "a")?.name).toBe("a");
    expect(findAgent(agents, "b")).toBeNull();
    expect(findAgent(agents, null)).toBeNull();
  });
});
