import { createContext, useContext, useMemo, type ReactNode } from "react";
import { POLL_MS, useAgents, type AgentsState } from "./useAgents";
import { useDeathNotifications } from "./useDeathNotifications";

const AgentsContext = createContext<AgentsState | null>(null);

export interface AgentsProviderProps {
  readonly children: ReactNode;
  readonly intervalMs?: number;
}

export function AgentsProvider({ children, intervalMs = POLL_MS }: AgentsProviderProps) {
  const { agents, loading, error, refresh } = useAgents(intervalMs);
  useDeathNotifications(agents, loading);
  const value = useMemo(
    () => ({ agents, loading, error, refresh }),
    [agents, loading, error, refresh],
  );
  return <AgentsContext value={value}>{children}</AgentsContext>;
}

export function useAgentsContext(): AgentsState {
  const state = useContext(AgentsContext);
  if (!state) throw new Error("useAgentsContext needs an AgentsProvider");
  return state;
}
