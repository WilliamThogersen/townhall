import { attachUrl } from "../api/client";

export type AttachStatus = "connecting" | "open" | "reconnecting" | "closed";

export interface AttachState {
  readonly status: AttachStatus;
  readonly error: string | null;
}

/** The close reason the daemon gives when the tmux session is gone. */
export const SESSION_ENDED = "session ended";
/** What the pane head reads as "ended: session closed". */
export const SESSION_GONE_ERROR = "session not found";

const BASE_DELAY_MS = 500;
const MAX_DELAY_MS = 10_000;

export const backoff = (attempt: number): number =>
  Math.min(MAX_DELAY_MS, BASE_DELAY_MS * 2 ** attempt);

export interface AttachHandlers {
  readonly onState: (state: AttachState) => void;
  /** The socket opened: the terminal should start from a clean screen and report its size. */
  readonly onOpen: () => void;
  readonly onOutput: (bytes: Uint8Array) => void;
  /** Whether a lost socket should be retried (the agent is still alive). */
  readonly shouldRetry: () => boolean;
}

export interface AttachConnection {
  readonly send: (bytes: Uint8Array<ArrayBuffer>) => void;
  readonly resize: (cols: number, rows: number) => void;
  readonly close: () => void;
}

/**
 * A live connection to the agent's tmux session. Output is binary, input is binary,
 * a resize is one JSON text frame. A lost socket is retried with backoff while
 * `shouldRetry` holds; the daemon closing with "session ended" stops for good.
 */
export function connectAttach(name: string, handlers: AttachHandlers): AttachConnection {
  let ws: WebSocket | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let attempt = 0;
  let stopped = false;

  const open = () => {
    const socket = new WebSocket(attachUrl(name));
    ws = socket;
    socket.binaryType = "arraybuffer";
    socket.onopen = () => {
      attempt = 0;
      handlers.onState({ status: "open", error: null });
      handlers.onOpen();
    };
    socket.onmessage = (ev: MessageEvent) => {
      if (ev.data instanceof ArrayBuffer) handlers.onOutput(new Uint8Array(ev.data));
    };
    socket.onclose = (ev: CloseEvent) => {
      if (stopped || ws !== socket) return;
      if (ev.reason === SESSION_ENDED) {
        handlers.onState({ status: "closed", error: SESSION_GONE_ERROR });
      } else if (handlers.shouldRetry()) {
        handlers.onState({ status: "reconnecting", error: null });
        timer = setTimeout(open, backoff(attempt++));
      } else {
        handlers.onState({ status: "closed", error: null });
      }
    };
  };

  handlers.onState({ status: "connecting", error: null });
  open();
  return {
    send: (bytes) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(bytes);
    },
    resize: (cols, rows) => {
      if (ws?.readyState === WebSocket.OPEN) ws.send(JSON.stringify({ type: "resize", cols, rows }));
    },
    close: () => {
      stopped = true;
      clearTimeout(timer);
      ws?.close();
    },
  };
}
