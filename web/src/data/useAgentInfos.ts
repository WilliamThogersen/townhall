import { useMemo } from "react";
import type { AgentInfo } from "../api/types";
import { useResource } from "./dataContext";
import { AGENTS_KEY } from "./resources";
import type { AgentInfosState } from "./types";

const NO_AGENTS: readonly AgentInfo[] = [];

export function useAgentInfos(): AgentInfosState {
  const { data, loading, error } = useResource(AGENTS_KEY, (state) => state.agents);
  return useMemo(() => ({ agents: data ?? NO_AGENTS, loading, error }), [data, loading, error]);
}
