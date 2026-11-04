import { describe, expect, it } from "vitest";
import type { Agent } from "../api/types";
import { agentStateOf, countByState } from "./agentState";

function agent(name: string, alive: boolean): Agent {
  return { name, kind: "shell", branch: `townhall/${name}`, worktree: `/w/${name}`, alive };
}

describe("agentStateOf", () => {
  it("maps a live agent to alive", () => {
    expect(agentStateOf(agent("a", true))).toBe("alive");
  });

  it("maps a stopped agent to dead", () => {
    expect(agentStateOf(agent("a", false))).toBe("dead");
  });
});

describe("countByState", () => {
  it("counts agents per state and leaves out absent states", () => {
    const agents = [agent("a", true), agent("b", false), agent("c", true), agent("d", true)];
    expect(countByState(agents)).toEqual({ alive: 3, dead: 1 });
  });

  it("returns an empty count for no agents", () => {
    expect(countByState([])).toEqual({});
  });

  it("does not change the list it counts", () => {
    const agents = Object.freeze([agent("a", true), agent("b", false)]);
    expect(() => countByState(agents)).not.toThrow();
    expect(agents).toHaveLength(2);
  });
});
