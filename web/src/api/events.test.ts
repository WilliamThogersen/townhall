import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  connectEvents,
  eventsBackoff,
  eventsUrl,
  openWebSocket,
  readFrame,
  type ConnectionStatus,
  type SocketHandlers,
} from "./events";
import { aConversation } from "./fixtures";
import type { ServerEvent } from "./types";

interface FakeSocket {
  readonly url: string;
  readonly handlers: SocketHandlers;
  readonly close: ReturnType<typeof vi.fn>;
}

function harness() {
  const sockets: FakeSocket[] = [];
  const statuses: ConnectionStatus[] = [];
  const received: ServerEvent[] = [];
  const connection = connectEvents((event) => received.push(event), {
    url: "ws://h/api/events",
    onStatus: (status) => statuses.push(status),
    openSocket: (url, handlers) => {
      const socket = { url, handlers, close: vi.fn() };
      sockets.push(socket);
      return socket;
    },
  });
  const last = () => sockets.at(-1)!;
  return { sockets, statuses, received, connection, last };
}

const updated = JSON.stringify({ type: "conversationUpdated", conversation: aConversation() });

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("events url", () => {
  it("follows the page in a browser and the loopback daemon on desktop", () => {
    expect(eventsUrl({ protocol: "https:", host: "h:1" }, {})).toBe("wss://h:1/api/events");
    expect(eventsUrl({ protocol: "http:", host: "h:1" }, {})).toBe("ws://h:1/api/events");
    expect(eventsUrl({ protocol: "http:", host: "h:1" }, { __TAURI_INTERNALS__: {} })).toBe(
      "ws://127.0.0.1:7777/api/events",
    );
  });
});

describe("readFrame", () => {
  it("parses a valid event and skips anything else", () => {
    expect(readFrame(updated)).toEqual({ type: "conversationUpdated", conversation: aConversation() });
    expect(readFrame("{not json")).toBeNull();
    expect(readFrame(JSON.stringify({ type: "later" }))).toBeNull();
    expect(readFrame(new ArrayBuffer(2))).toBeNull();
  });
});

describe("connectEvents", () => {
  it("reports connecting then open and passes parsed events on", () => {
    const h = harness();
    expect(h.last().url).toBe("ws://h/api/events");
    h.last().handlers.onOpen();
    h.last().handlers.onMessage(updated);
    h.last().handlers.onMessage("garbage");
    h.last().handlers.onMessage(JSON.stringify({ type: "resync" }));
    expect(h.statuses).toEqual(["connecting", "open"]);
    expect(h.received.map((e) => e.type)).toEqual(["conversationUpdated", "resync"]);
  });

  it("reconnects with a growing delay and resets it once open", () => {
    const h = harness();
    h.last().handlers.onClose();
    expect(h.statuses).toEqual(["connecting", "closed"]);
    vi.advanceTimersByTime(499);
    expect(h.sockets).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(h.sockets).toHaveLength(2);
    h.last().handlers.onClose();
    vi.advanceTimersByTime(999);
    expect(h.sockets).toHaveLength(2);
    vi.advanceTimersByTime(1);
    expect(h.sockets).toHaveLength(3);
    h.last().handlers.onOpen();
    h.last().handlers.onClose();
    vi.advanceTimersByTime(500);
    expect(h.sockets).toHaveLength(4);
    expect(h.statuses.slice(-3)).toEqual(["open", "closed", "connecting"]);
  });

  it("caps the delay", () => {
    expect(eventsBackoff(0, 500, 10_000)).toBe(500);
    expect(eventsBackoff(3, 500, 10_000)).toBe(4000);
    expect(eventsBackoff(10, 500, 10_000)).toBe(10_000);
  });

  it("retries when the socket cannot be made", () => {
    const statuses: ConnectionStatus[] = [];
    const openSocket = vi.fn(() => {
      throw new Error("bad url");
    });
    const connection = connectEvents(() => undefined, {
      url: "x",
      onStatus: (s) => statuses.push(s),
      openSocket,
      baseDelayMs: 10,
    });
    vi.advanceTimersByTime(10);
    expect(openSocket).toHaveBeenCalledTimes(2);
    expect(statuses).toEqual(["connecting", "closed", "connecting", "closed"]);
    connection.close();
  });

  it("stops for good on close", () => {
    const h = harness();
    h.connection.close();
    expect(h.last().close).toHaveBeenCalledOnce();
    h.last().handlers.onOpen();
    h.last().handlers.onMessage(updated);
    h.last().handlers.onClose();
    vi.advanceTimersByTime(60_000);
    expect(h.sockets).toHaveLength(1);
    expect(h.statuses).toEqual(["connecting"]);
    expect(h.received).toEqual([]);
  });

  it("clears a pending retry on close", () => {
    const h = harness();
    h.last().handlers.onClose();
    h.connection.close();
    vi.advanceTimersByTime(60_000);
    expect(h.sockets).toHaveLength(1);
  });
});

describe("openWebSocket", () => {
  it("wires a browser socket to the handlers", () => {
    const made: FakeWs[] = [];
    class FakeWs {
      onopen: (() => void) | null = null;
      onmessage: ((e: { data: unknown }) => void) | null = null;
      onclose: (() => void) | null = null;
      readonly close = vi.fn();
      constructor(readonly url: string) {
        made.push(this);
      }
    }
    vi.stubGlobal("WebSocket", FakeWs);
    const handlers = { onOpen: vi.fn(), onMessage: vi.fn(), onClose: vi.fn() };
    const socket = openWebSocket("ws://h/api/events", handlers);
    const ws = made[0]!;
    ws.onopen?.();
    ws.onmessage?.({ data: "frame" });
    ws.onclose?.();
    socket.close();
    expect(ws.url).toBe("ws://h/api/events");
    expect(handlers.onOpen).toHaveBeenCalledOnce();
    expect(handlers.onMessage).toHaveBeenCalledWith("frame");
    expect(handlers.onClose).toHaveBeenCalledOnce();
    expect(ws.close).toHaveBeenCalledOnce();
  });
});
