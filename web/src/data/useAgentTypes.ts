import { useMemo } from "react";
import { listConversations } from "../api/conversations";
import { getAgentType, getAgentUsageSeries, listAgentRuns, listAgentTypes, listAgentUsage } from "../api/library";
import type { AgentRun, AgentType, AgentTypeSummary, AgentUsage, AgentUsageSeries, UsageRange } from "../api/types";
import { useLoad, type Loaded } from "./useLoad";

const NO_REFRESH = 0;
const NO_TITLES: ReadonlyMap<string, string> = new Map();

export function useAgentTypes(): Loaded<readonly AgentTypeSummary[]> {
  return useLoad("agent-types", listAgentTypes, NO_REFRESH);
}

export function useAgentType(name: string | null): Loaded<AgentType> {
  return useLoad(name === null ? null : `agent-type:${name}`, (signal) => getAgentType(name ?? "", signal), NO_REFRESH);
}

export function useAgentUsageSeries(name: string, range: UsageRange): Loaded<AgentUsageSeries> {
  return useLoad(`agent-usage-series:${name}:${range}`, (signal) => getAgentUsageSeries(name, range, signal), NO_REFRESH);
}

export function useAgentRuns(name: string): Loaded<readonly AgentRun[]> {
  return useLoad(`agent-runs:${name}`, (signal) => listAgentRuns(name, signal), NO_REFRESH);
}

export function useAgentUsage(): Loaded<readonly AgentUsage[]> {
  return useLoad("agent-usage", listAgentUsage, NO_REFRESH);
}

export function useConversationTitles(): ReadonlyMap<string, string> {
  const { data } = useLoad("conversation-titles", listConversations, NO_REFRESH);
  return useMemo(() => (data === null ? NO_TITLES : new Map(data.map((c) => [c.id, c.title]))), [data]);
}
