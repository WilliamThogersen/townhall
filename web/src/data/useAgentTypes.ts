import { getAgentType, listAgentTypes } from "../api/library";
import type { AgentType, AgentTypeSummary } from "../api/types";
import { useLoad, type Loaded } from "./useLoad";

const NO_REFRESH = 0;

export function useAgentTypes(): Loaded<readonly AgentTypeSummary[]> {
  return useLoad("agent-types", listAgentTypes, NO_REFRESH);
}

export function useAgentType(name: string | null): Loaded<AgentType> {
  return useLoad(name === null ? null : `agent-type:${name}`, (signal) => getAgentType(name ?? "", signal), NO_REFRESH);
}
