import type { Agent } from "../api/types";
import type { AgentState, AgentStateCounts } from "../ui/StatusGlyph";

export function agentStateOf(agent: Agent): AgentState {
  return agent.alive ? "alive" : "dead";
}

export function countByState(agents: readonly Agent[]): AgentStateCounts {
  return agents.reduce<AgentStateCounts>((counts, agent) => {
    const state = agentStateOf(agent);
    return { ...counts, [state]: (counts[state] ?? 0) + 1 };
  }, {});
}
