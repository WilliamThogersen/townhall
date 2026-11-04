import { countByState } from "../../agents/agentState";
import type { Agent } from "../../api/types";

export interface ControlStat {
  readonly label: string;
  readonly value: number;
}

function byAliveThenName(a: Agent, b: Agent): number {
  if (a.alive !== b.alive) return a.alive ? -1 : 1;
  return a.name.localeCompare(b.name);
}

export function sortAgents(agents: readonly Agent[]): readonly Agent[] {
  return [...agents].sort(byAliveThenName);
}

export function controlStats(agents: readonly Agent[]): readonly ControlStat[] {
  const counts = countByState(agents);
  return [
    { label: "agents", value: agents.length },
    { label: "running", value: counts.alive ?? 0 },
    { label: "ended", value: counts.dead ?? 0 },
  ];
}

export function findAgent(agents: readonly Agent[], name: string | null): Agent | null {
  if (name === null) return null;
  return agents.find((agent) => agent.name === name) ?? null;
}
