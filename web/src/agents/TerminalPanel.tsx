import { useAgentStream } from "./useAgentStream";
import { lazy, Suspense } from "react";

const Terminal = lazy(() => import("./Terminal").then((m) => ({ default: m.Terminal })));
import { SendBox } from "./SendBox";

/** Mounted only for opened agents, so only they hold a WebSocket. */
export function TerminalPanel({ name }: { name: string }) {
  const { snapshot, status, error } = useAgentStream(name, true);
  return (
    <div className="panel">
      <p className="stream-status" data-status={status}>
        {error ? `Session ended: ${error}` : status === "open" ? "Live" : `Stream ${status}...`}
      </p>
      <Suspense fallback={<p className="muted">Loading terminal...</p>}>
        <Terminal snapshot={snapshot} />
      </Suspense>
      <SendBox name={name} />
    </div>
  );
}
