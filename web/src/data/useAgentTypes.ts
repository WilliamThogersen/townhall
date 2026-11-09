import { getAgentType, listAgentRuns, listAgentTypes, listAgentUsage } from "../api/library";
import type { AgentRun, AgentType, AgentTypeSummary, AgentUsage } from "../api/types";
import { useLoad, type Loaded } from "./useLoad";

const NO_REFRESH = 0;

export function useAgentTypes(): Loaded<readonly AgentTypeSummary[]> {
  return useLoad("agent-types", listAgentTypes, NO_REFRESH);
}

export function useAgentType(name: string | null): Loaded<AgentType> {
  return useLoad(name === null ? null : `agent-type:${name}`, (signal) => getAgentType(name ?? "", signal), NO_REFRESH);
}

export function useAgentRuns(name: string): Loaded<readonly AgentRun[]> {
  return useLoad(`agent-runs:${name}`, (signal) => listAgentRuns(name, signal), NO_REFRESH);
}

export function useAgentUsage(): Loaded<readonly AgentUsage[]> {
  return useLoad("agent-usage", listAgentUsage, NO_REFRESH);
}
