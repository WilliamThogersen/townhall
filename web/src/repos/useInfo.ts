import { useCallback, useEffect, useState } from "react";
import { getInfo } from "../api/client";
import { toMessage } from "../api/errors";
import type { Info } from "../api/types";

interface InfoSnapshot {
  readonly info: Info | null;
  readonly loading: boolean;
  readonly error: string | null;
}

export interface InfoState extends InfoSnapshot {
  readonly reload: () => void;
}

const STARTING: InfoSnapshot = { info: null, loading: true, error: null };

export function useInfo(): InfoState {
  const [snapshot, setSnapshot] = useState<InfoSnapshot>(STARTING);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    const abort = new AbortController();
    getInfo(abort.signal)
      .then((info) => setSnapshot({ info, loading: false, error: null }))
      .catch((e: unknown) => {
        if (abort.signal.aborted) return;
        setSnapshot((prev) => ({ info: prev.info, loading: false, error: toMessage(e) }));
      });
    return () => abort.abort();
  }, [attempt]);

  const reload = useCallback(() => {
    setSnapshot((prev) => ({ ...prev, loading: true }));
    setAttempt((n) => n + 1);
  }, []);

  return { ...snapshot, reload };
}
