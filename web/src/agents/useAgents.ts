import { useCallback, useEffect, useRef, useState } from "react";
import { listAgents } from "../api/client";
import { toMessage } from "../api/errors";
import type { Agent } from "../api/types";

export const POLL_MS = 3000;

export interface AgentsState {
  readonly agents: readonly Agent[];
  readonly loading: boolean;
  readonly error: string | null;
  readonly refresh: () => Promise<void>;
}

/** Polls GET /agents. Keeps the last good list when a poll fails. */
export function useAgents(intervalMs: number = POLL_MS): AgentsState {
  const [agents, setAgents] = useState<readonly Agent[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const alive = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const next = await listAgents();
      if (!alive.current) return;
      setAgents(next);
      setError(null);
    } catch (e) {
      if (alive.current) setError(toMessage(e));
    } finally {
      if (alive.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    alive.current = true;
    void refresh();
    const id = setInterval(() => void refresh(), intervalMs);
    return () => {
      alive.current = false;
      clearInterval(id);
    };
  }, [refresh, intervalMs]);

  return { agents, loading, error, refresh };
}
