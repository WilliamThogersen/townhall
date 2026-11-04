import { useEffect, useMemo } from "react";
import { getSteps } from "../api/steps";
import type { Step } from "../api/types";
import { useGeneration } from "./dataContext";
import type { StepsOptions, StepsState } from "./types";
import { useLoad } from "./useLoad";

export const STEPS_POLL_MS = 2000;

const NO_STEPS: readonly Step[] = [];

export function useSteps(agent: string | null, options: StepsOptions = {}): StepsState {
  const generation = useGeneration();
  const { live = false, repo, intervalMs = STEPS_POLL_MS } = options;
  const { data, loading, error, reload } = useLoad(
    agent === null ? null : JSON.stringify(["steps", agent, repo ?? null]),
    (signal) => getSteps(agent ?? "", repo, signal),
    generation,
  );

  useEffect(() => {
    if (!live || agent === null) return;
    const id = setInterval(reload, intervalMs);
    return () => clearInterval(id);
  }, [live, agent, intervalMs, reload]);

  return useMemo(() => ({ steps: data ?? NO_STEPS, loading, error }), [data, loading, error]);
}
