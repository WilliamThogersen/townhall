import { useEffect, useMemo } from "react";
import { getUsage } from "../api/usage";
import { useGeneration } from "./dataContext";
import type { UsageState } from "./types";
import { useLoad } from "./useLoad";

export const USAGE_POLL_MS = 60_000;

export function useUsage(): UsageState {
  const generation = useGeneration();
  const { data, loading, error, reload } = useLoad("usage", (signal) => getUsage(signal), generation);

  useEffect(() => {
    const id = setInterval(reload, USAGE_POLL_MS);
    return () => clearInterval(id);
  }, [reload]);

  return useMemo(() => ({ usage: data, loading, error }), [data, loading, error]);
}
