import type { AttachState } from "./attachSocket";

export type PaneStreamStatus = "open" | "connecting" | "ended" | "error";

export interface PaneStreamView {
  readonly status: PaneStreamStatus;
  readonly glyph: "●" | "○" | "×" | null;
  readonly label: string;
}

const LIVE: PaneStreamView = { status: "open", glyph: "●", label: "live" };
const CONNECTING: PaneStreamView = { status: "connecting", glyph: null, label: "connecting" };
const RECONNECTING: PaneStreamView = { status: "connecting", glyph: null, label: "reconnecting" };
const ENDED: PaneStreamView = { status: "ended", glyph: "○", label: "ended: session closed" };

const SESSION_GONE = /not found/i;

const closedView = (reason: string): PaneStreamView =>
  SESSION_GONE.test(reason) ? ENDED : { status: "error", glyph: "×", label: `error: ${reason}` };

export function paneStreamView(
  stream: AttachState,
  alive: boolean,
): PaneStreamView {
  if (stream.error !== null) return closedView(stream.error);
  if (!alive || stream.status === "closed") return ENDED;
  if (stream.status === "open") return LIVE;
  return stream.status === "reconnecting" ? RECONNECTING : CONNECTING;
}
