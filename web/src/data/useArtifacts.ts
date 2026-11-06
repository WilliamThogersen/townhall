import { useEffect, useMemo } from "react";
import { listArtifacts } from "../api/artifacts";
import type { Artifact } from "../api/types";
import type { ArtifactsState } from "./types";
import { useLoad } from "./useLoad";

export const ARTIFACTS_POLL_MS = 5000;

const NO_ARTIFACTS: readonly Artifact[] = [];

export function useArtifacts(task: string | null, intervalMs = ARTIFACTS_POLL_MS): ArtifactsState {
  const { data, loading, error, reload } = useLoad(
    task === null ? null : JSON.stringify(["artifacts", task]),
    (signal) => listArtifacts(task ?? "", signal),
    0,
  );

  useEffect(() => {
    if (task === null) return;
    const id = setInterval(reload, intervalMs);
    return () => clearInterval(id);
  }, [task, intervalMs, reload]);

  return useMemo(() => ({ artifacts: data ?? NO_ARTIFACTS, loading, error }), [data, loading, error]);
}
