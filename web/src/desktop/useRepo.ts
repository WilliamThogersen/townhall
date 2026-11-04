import { useCallback, useEffect, useRef, useState } from "react";
import { toMessage } from "../api/errors";
import { chooseRepo, repoStatus, type RepoStatus } from "./bridge";

export interface RepoState {
  readonly status: RepoStatus | null;
  readonly busy: boolean;
  readonly error: string | null;
  readonly choose: () => Promise<void>;
}

/** Repo selection for the desktop shell. Opens the picker once on first launch. */
export function useRepo(): RepoState {
  const [status, setStatus] = useState<RepoStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const autoPicked = useRef(false);

  const choose = useCallback(async () => {
    setBusy(true);
    setError(null);
    try {
      setStatus(await chooseRepo());
    } catch (e) {
      setError(toMessage(e));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    let live = true;
    repoStatus()
      .then((s) => {
        if (!live) return;
        setStatus(s);
        if (s.repo === null && s.mode === "stopped" && !autoPicked.current) {
          autoPicked.current = true;
          void choose();
        }
      })
      .catch((e: unknown) => live && setError(toMessage(e)));
    return () => {
      live = false;
    };
  }, [choose]);

  return { status, busy, error, choose };
}
