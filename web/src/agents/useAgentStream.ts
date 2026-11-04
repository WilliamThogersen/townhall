import { useEffect, useState } from "react";
import { streamUrl } from "../api/client";

export type StreamStatus = "connecting" | "open" | "reconnecting" | "closed";

export interface StreamState {
  readonly snapshot: string;
  readonly status: StreamStatus;
  readonly error: string | null;
}

const BASE_DELAY_MS = 500;
const MAX_DELAY_MS = 10_000;
const ERROR_PREFIX = "error: ";

export const backoff = (attempt: number): number =>
  Math.min(MAX_DELAY_MS, BASE_DELAY_MS * 2 ** attempt);

const INITIAL: StreamState = { snapshot: "", status: "connecting", error: null };

/**
 * Live pane snapshots for one agent. Each message is a full capture.
 * Reconnects with exponential backoff; a server "error: ..." message
 * means the session is gone, so we surface it and stop retrying.
 */
export function useAgentStream(name: string, enabled: boolean): StreamState {
  const [state, setState] = useState<StreamState>(INITIAL);

  useEffect(() => {
    if (!enabled) return;
    let ws: WebSocket | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let attempt = 0;
    let stopped = false;
    let fatal = false;

    const connect = () => {
      ws = new WebSocket(streamUrl(name));
      ws.onopen = () => {
        attempt = 0;
        setState((s) => ({ ...s, status: "open", error: null }));
      };
      ws.onmessage = (ev: MessageEvent) => {
        const text = typeof ev.data === "string" ? ev.data : "";
        if (text.startsWith(ERROR_PREFIX)) {
          fatal = true;
          setState((s) => ({ ...s, status: "closed", error: text.slice(ERROR_PREFIX.length) }));
          return;
        }
        setState((s) => ({ ...s, snapshot: text }));
      };
      ws.onclose = () => {
        if (stopped || fatal) return;
        setState((s) => ({ ...s, status: "reconnecting" }));
        timer = setTimeout(connect, backoff(attempt++));
      };
    };

    setState(INITIAL);
    connect();
    return () => {
      stopped = true;
      clearTimeout(timer);
      ws?.close();
    };
  }, [name, enabled]);

  return state;
}
