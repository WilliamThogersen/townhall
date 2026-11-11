import { useEffect, useMemo, useRef } from "react";
import { listRuns } from "../api/runs";
import type { Run } from "../api/types";
import type { RunsState } from "./types";
import { useLoad } from "./useLoad";

export const RUNS_POLL_MS = 5000;

const NO_RUNS: readonly Run[] = [];

export function useRuns(conversationId: string | null, watch: string, intervalMs = RUNS_POLL_MS): RunsState {
  const { data, loading, error, reload } = useLoad(
    conversationId === null ? null : JSON.stringify(["runs", conversationId]),
    (signal) => listRuns(conversationId ?? "", signal),
    0,
  );

  useEffect(() => {
    if (conversationId === null) return;
    const id = setInterval(reload, intervalMs);
    return () => clearInterval(id);
  }, [conversationId, intervalMs, reload]);

  const seen = useRef(watch);
  useEffect(() => {
    if (seen.current === watch) return;
    seen.current = watch;
    reload();
  }, [watch, reload]);

  return useMemo(() => ({ runs: data ?? NO_RUNS, loading, error }), [data, loading, error]);
}
