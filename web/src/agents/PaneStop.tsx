import { useEffect, useRef, useState, type KeyboardEvent, type RefObject } from "react";
import { stopAgent } from "../api/client";
import { toMessage } from "../api/errors";
import { Button } from "../ui/Button";

export interface StopConfirm {
  readonly confirming: boolean;
  readonly busy: boolean;
  readonly error: string | null;
  readonly stopButton: RefObject<HTMLButtonElement | null>;
  readonly ask: () => void;
  readonly cancel: () => void;
  readonly confirm: () => Promise<void>;
  readonly onKeyDown: (e: KeyboardEvent) => void;
}

interface StopState {
  readonly confirming: boolean;
  readonly busy: boolean;
  readonly error: string | null;
}

const IDLE: StopState = { confirming: false, busy: false, error: null };
const ASKING: StopState = { confirming: true, busy: false, error: null };

function useFocusOnClose(confirming: boolean, target: RefObject<HTMLButtonElement | null>) {
  const wasConfirming = useRef(false);
  useEffect(() => {
    if (wasConfirming.current && !confirming) target.current?.focus();
    wasConfirming.current = confirming;
  }, [confirming, target]);
}

export function useStopConfirm(name: string, onStopped: () => void): StopConfirm {
  const [state, setState] = useState<StopState>(IDLE);
  const stopButton = useRef<HTMLButtonElement>(null);
  useFocusOnClose(state.confirming, stopButton);

  const cancel = () => setState(IDLE);

  const confirm = async () => {
    setState({ confirming: true, busy: true, error: null });
    try {
      await stopAgent(name);
      setState(IDLE);
      onStopped();
    } catch (e) {
      setState({ confirming: true, busy: false, error: toMessage(e) });
    }
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || state.busy) return;
    e.stopPropagation();
    cancel();
  };

  return { ...state, stopButton, ask: () => setState(ASKING), cancel, confirm, onKeyDown };
}

export function PaneStopConfirm({ name, stop }: { name: string; stop: StopConfirm }) {
  const cancelButton = useRef<HTMLButtonElement>(null);

  useEffect(() => {
    cancelButton.current?.focus();
  }, [stop.error]);

  return (
    <div className="th-pane-actions">
      <span className="th-confirm">
        {stop.error ? (
          <span role="alert">error: {stop.error}</span>
        ) : (
          <span>stop {name} and remove its worktree?</span>
        )}
      </span>
      <Button ref={cancelButton} variant="ghost" size="sm" onClick={stop.cancel} disabled={stop.busy}>
        cancel
      </Button>
      <Button
        variant="danger-solid"
        size="sm"
        onClick={() => void stop.confirm()}
        busy={stop.busy}
        busyLabel="stopping…"
      >
        stop and remove
      </Button>
    </div>
  );
}
