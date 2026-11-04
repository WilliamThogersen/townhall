import { useState } from "react";
import { stopAgent } from "../api/client";
import { toMessage } from "../api/errors";

export function StopButton({ name, onStopped }: { name: string; onStopped: () => void }) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const stop = async () => {
    setBusy(true);
    setError(null);
    try {
      await stopAgent(name);
      onStopped();
    } catch (e) {
      setError(toMessage(e));
      setBusy(false);
    }
  };

  if (!confirming) {
    return (
      <button className="danger" onClick={() => setConfirming(true)}>
        Stop
      </button>
    );
  }
  return (
    <span className="confirm">
      <span>Stop {name} and remove its worktree?</span>
      <button className="danger" onClick={() => void stop()} disabled={busy}>
        {busy ? "Stopping..." : "Confirm"}
      </button>
      <button onClick={() => setConfirming(false)} disabled={busy}>
        Cancel
      </button>
      {error && <span className="error" role="alert">{error}</span>}
    </span>
  );
}
