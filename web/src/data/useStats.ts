import { useMemo } from "react";
import { getStats } from "../api/stats";
import { useGeneration } from "./dataContext";
import type { StatsState } from "./types";
import { useLoad } from "./useLoad";

export function useStats(repo?: string): StatsState {
  const generation = useGeneration();
  const { data, loading, error } = useLoad(
    JSON.stringify(["stats", repo ?? null]),
    (signal) => getStats(repo, signal),
    generation,
  );
  return useMemo(() => ({ stats: data, loading, error }), [data, loading, error]);
}
