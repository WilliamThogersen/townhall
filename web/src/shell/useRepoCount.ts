import { useEffect, useState } from "react";
import { getInfo } from "../api/client";

const REPOS_PER_DAEMON = 1;

export function useRepoCount(daemonReachable: boolean): number | null {
  const [count, setCount] = useState<number | null>(null);
  useEffect(() => {
    if (!daemonReachable || count !== null) return;
    const abort = new AbortController();
    getInfo(abort.signal)
      .then(() => setCount(REPOS_PER_DAEMON))
      .catch(() => setCount(null));
    return () => abort.abort();
  }, [daemonReachable, count]);
  return count;
}
