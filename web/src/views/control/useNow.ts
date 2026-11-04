import { useEffect, useState } from "react";

export const NOW_TICK_MS = 15_000;

export function useNow(intervalMs: number = NOW_TICK_MS): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const id = setInterval(() => setNow(Date.now()), intervalMs);
    return () => clearInterval(id);
  }, [intervalMs]);
  return now;
}
