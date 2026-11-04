import { listAgents } from "../api/client";
import type { AgentInfo } from "../api/types";
import { useDataContext } from "./dataContext";
import type { AgentInfosState } from "./types";
import { useLoad } from "./useLoad";

const NO_AGENTS: readonly AgentInfo[] = [];

export function useAgentInfos(): AgentInfosState {
  const { refresh } = useDataContext();
  const { data, loading, error } = useLoad("agents", (signal) => listAgents(signal), refresh);
  return { agents: data ?? NO_AGENTS, loading, error };
}
