import { wsApiBase, type PageLocation } from "./base";
import { parseServerEvent } from "./eventGuards";
import type { ServerEvent } from "./types";

export type ConnectionStatus = "connecting" | "open" | "closed";

export const EVENTS_BASE_DELAY_MS = 500;
export const EVENTS_MAX_DELAY_MS = 10_000;

export interface SocketHandlers {
  readonly onOpen: () => void;
  readonly onMessage: (data: unknown) => void;
  readonly onClose: () => void;
}

export interface EventSocket {
  readonly close: () => void;
}

export type OpenSocket = (url: string, handlers: SocketHandlers) => EventSocket;

export interface EventsOptions {
  readonly onStatus?: (status: ConnectionStatus) => void;
  readonly url?: string;
  readonly openSocket?: OpenSocket;
  readonly baseDelayMs?: number;
  readonly maxDelayMs?: number;
}

export interface EventsConnection {
  readonly close: () => void;
}

export type ConnectEvents = (
  onEvent: (event: ServerEvent) => void,
  options?: EventsOptions,
) => EventsConnection;

export function eventsUrl(loc: PageLocation = window.location, win: object = window): string {
  return `${wsApiBase(loc, win)}/events`;
}

export const openWebSocket: OpenSocket = (url, handlers) => {
  const ws = new WebSocket(url);
  ws.onopen = () => handlers.onOpen();
  ws.onmessage = (event: MessageEvent) => handlers.onMessage(event.data);
  ws.onclose = () => handlers.onClose();
  return { close: () => ws.close() };
};

export function readFrame(data: unknown): ServerEvent | null {
  if (typeof data !== "string") return null;
  try {
    return parseServerEvent(JSON.parse(data));
  } catch {
    return null;
  }
}

export function eventsBackoff(attempt: number, baseMs: number, maxMs: number): number {
  return Math.min(maxMs, baseMs * 2 ** attempt);
}

const ignoreStatus = (): void => undefined;

export const connectEvents: ConnectEvents = (onEvent, options = {}) => {
  const onStatus = options.onStatus ?? ignoreStatus;
  const url = options.url ?? eventsUrl();
  const openSocket = options.openSocket ?? openWebSocket;
  const baseMs = options.baseDelayMs ?? EVENTS_BASE_DELAY_MS;
  const maxMs = options.maxDelayMs ?? EVENTS_MAX_DELAY_MS;
  let socket: EventSocket | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let attempt = 0;
  let stopped = false;

  const retry = () => {
    if (stopped) return;
    onStatus("closed");
    timer = setTimeout(connect, eventsBackoff(attempt, baseMs, maxMs));
    attempt += 1;
  };
  const handlers: SocketHandlers = {
    onOpen: () => {
      if (stopped) return;
      attempt = 0;
      onStatus("open");
    },
    onMessage: (data) => {
      const event = stopped ? null : readFrame(data);
      if (event) onEvent(event);
    },
    onClose: retry,
  };
  function connect() {
    onStatus("connecting");
    try {
      socket = openSocket(url, handlers);
    } catch {
      retry();
    }
  }

  connect();
  return {
    close: () => {
      stopped = true;
      clearTimeout(timer);
      socket?.close();
    },
  };
};
