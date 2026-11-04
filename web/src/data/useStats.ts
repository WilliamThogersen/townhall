import { getStats } from "../api/stats";
import { useDataContext } from "./dataContext";
import type { StatsState } from "./types";
import { useLoad } from "./useLoad";

export function useStats(repo?: string): StatsState {
  const { refresh } = useDataContext();
  const { data, loading, error } = useLoad(
    JSON.stringify(["stats", repo ?? null]),
    (signal) => getStats(repo, signal),
    refresh,
  );
  return { stats: data, loading, error };
}
