import { useEffect, useState } from "react";
import { Spinner } from "./Spinner";

export const LOADING_DELAY_MS = 300;

export interface LoadingProps {
  readonly verb: string;
  readonly delayMs?: number;
}

export function Loading({ verb, delayMs = LOADING_DELAY_MS }: LoadingProps) {
  const [elapsed, setElapsed] = useState(false);

  useEffect(() => {
    if (delayMs <= 0) return;
    const timer = setTimeout(() => setElapsed(true), delayMs);
    return () => clearTimeout(timer);
  }, [delayMs]);

  if (delayMs > 0 && !elapsed) return null;
  return (
    <span className="th-loading" role="status">
      <Spinner />
      {verb}…
    </span>
  );
}
