import { useEffect, useRef } from "react";
import type { Agent } from "../api/types";
import { notifyAgentDied } from "../desktop/notify";

/** Names alive in `prev` that are present but dead in `next`. Removed agents are not deaths. */
export function findDeaths(prev: readonly Agent[], next: readonly Agent[]): string[] {
  const wasAlive = new Set(prev.filter((a) => a.alive).map((a) => a.name));
  return next.filter((a) => !a.alive && wasAlive.has(a.name)).map((a) => a.name);
}

/** Fires a notification when a polled agent goes from alive to dead. */
export function useDeathNotifications(agents: readonly Agent[], loading: boolean): void {
  const prev = useRef<readonly Agent[] | null>(null);
  useEffect(() => {
    if (loading) return;
    if (prev.current) {
      for (const name of findDeaths(prev.current, agents)) void notifyAgentDied(name);
    }
    prev.current = agents;
  }, [agents, loading]);
}
