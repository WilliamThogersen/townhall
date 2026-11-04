import { useCallback, useEffect, useRef, useState } from "react";
import { toMessage } from "../api/errors";

export type Load<T> = (signal: AbortSignal) => Promise<T>;

export interface Loaded<T> {
  readonly data: T | null;
  readonly loading: boolean;
  readonly error: string | null;
  readonly reload: () => void;
}

interface Snapshot<T> {
  readonly key: string | null;
  readonly data: T | null;
  readonly error: string | null;
  readonly settled: boolean;
}

export function useLoad<T>(key: string | null, load: Load<T>, refresh: number): Loaded<T> {
  const latest = useRef(load);
  const [snapshot, setSnapshot] = useState<Snapshot<T>>({ key, data: null, error: null, settled: false });
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    latest.current = load;
  });

  useEffect(() => {
    if (key === null) return;
    const abort = new AbortController();
    latest.current(abort.signal).then(
      (data) => {
        if (!abort.signal.aborted) setSnapshot({ key, data, error: null, settled: true });
      },
      (e: unknown) => {
        if (abort.signal.aborted) return;
        setSnapshot((prev) => ({
          key,
          data: prev.key === key ? prev.data : null,
          error: toMessage(e),
          settled: true,
        }));
      },
    );
    return () => abort.abort();
  }, [key, refresh, attempt]);

  const reload = useCallback(() => setAttempt((n) => n + 1), []);
  const current = snapshot.key === key;
  return {
    data: current ? snapshot.data : null,
    loading: key !== null && !(current && snapshot.settled),
    error: current ? snapshot.error : null,
    reload,
  };
}
