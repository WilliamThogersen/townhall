import { getAgentType, getAgentUsageSeries, listAgentTypes, listAgentUsage } from "../api/library";
import type { AgentType, AgentTypeSummary, AgentUsage, AgentUsageSeries, UsageRange } from "../api/types";
import { useLoad, type Loaded } from "./useLoad";

const NO_REFRESH = 0;

export function useAgentTypes(): Loaded<readonly AgentTypeSummary[]> {
  return useLoad("agent-types", listAgentTypes, NO_REFRESH);
}

export function useAgentType(name: string | null): Loaded<AgentType> {
  return useLoad(name === null ? null : `agent-type:${name}`, (signal) => getAgentType(name ?? "", signal), NO_REFRESH);
}

export function useAgentUsageSeries(name: string, range: UsageRange): Loaded<AgentUsageSeries> {
  return useLoad(`agent-usage-series:${name}:${range}`, (signal) => getAgentUsageSeries(name, range, signal), NO_REFRESH);
}

export function useAgentUsage(): Loaded<readonly AgentUsage[]> {
  return useLoad("agent-usage", listAgentUsage, NO_REFRESH);
}
