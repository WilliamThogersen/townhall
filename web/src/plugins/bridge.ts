import { httpBase } from "../api/base";
import type { ConnectEvents, EventsConnection } from "../api/events";
import { toMessage } from "../api/errors";
import { isRecord } from "../api/guardKit";
import { parseEnvelope } from "../api/guards";
import { matchRoute } from "../router/match";
import { createRateLimiter } from "./rateLimit";
import { eventScopes, resolveCall } from "./scopes";

export const API_VERSION = 1;
export const TOAST_MAX_CHARS = 200;
export const DEFAULT_HEIGHT_RANGE: readonly [number, number] = [24, 400];

const TONES = ["info", "success", "warn", "error"] as const;

export type Tone = (typeof TONES)[number];

export interface HostSession {
  readonly plugin: string;
  readonly version: string | null;
  readonly token: string;
  readonly scopes: readonly string[];
}

export interface ThemePayload {
  readonly resolved: "dark" | "light";
  readonly tokens: Readonly<Record<string, string>>;
}

export interface BridgeUi {
  readonly resize: (height: number) => void;
  readonly toast: (text: string, tone: Tone) => void;
  readonly navigate: (path: string) => void;
  readonly ready: () => void;
  readonly paused: () => void;
  readonly failed: (why: string) => void;
}

export interface BridgeOptions {
  readonly slot: string;
  readonly context: Readonly<Record<string, unknown>>;
  readonly frame: () => Window | null;
  readonly session: () => Promise<HostSession>;
  readonly fetch: typeof fetch;
  readonly connectEvents: ConnectEvents;
  readonly theme: () => ThemePayload;
  readonly subscribeTheme: (listener: () => void) => () => void;
  readonly ui: BridgeUi;
  readonly limiter?: () => boolean;
  readonly heightRange?: readonly [number, number];
}

export interface FrameEvent {
  readonly source: unknown;
  readonly data: unknown;
}

export interface Bridge {
  readonly handle: (event: FrameEvent) => void;
  readonly dispose: () => void;
}

interface Message {
  readonly id: string | number | undefined;
  readonly type: string;
  readonly body: Readonly<Record<string, unknown>>;
}

type Answer = { readonly result: unknown } | { readonly error: string };

const fail = (error: string): Answer => ({ error });

function readMessage(data: unknown): Message | null {
  if (!isRecord(data) || data.th !== 1 || typeof data.type !== "string") return null;
  const id = typeof data.id === "string" || typeof data.id === "number" ? data.id : undefined;
  return { id, type: data.type, body: data };
}

function townhallRoute(path: unknown): path is string {
  if (typeof path !== "string" || !path.startsWith("/") || path.startsWith("//")) return false;
  const url = new URL(path, "http://townhall.invalid");
  return url.origin === "http://townhall.invalid" && matchRoute(url.pathname).name !== "not-found";
}

export function createBridge(options: BridgeOptions): Bridge {
  const limiter = options.limiter ?? createRateLimiter();
  const [minHeight, maxHeight] = options.heightRange ?? DEFAULT_HEIGHT_RANGE;
  let granted: HostSession | null = null;
  let stopped = false;
  let events: EventsConnection | null = null;
  let unsubscribeTheme: (() => void) | null = null;

  const send = (message: Record<string, unknown>) => {
    if (stopped) return;
    options.frame()?.postMessage({ th: 1, ...message }, "*");
  };
  const reply = (id: Message["id"], answer: Answer) => {
    if (id !== undefined) send({ type: "response", id, ...answer });
  };
  const stop = () => {
    stopped = true;
    events?.close();
    events = null;
    unsubscribeTheme?.();
    unsubscribeTheme = null;
  };

  const hello = async (message: Message) => {
    const version = message.body.apiVersion;
    if (version !== API_VERSION) {
      reply(message.id, fail(`plugin api version ${String(version)} is not supported, this host speaks ${API_VERSION}`));
      return;
    }
    let session: HostSession;
    try {
      session = await options.session();
    } catch (e) {
      options.ui.failed(toMessage(e));
      return;
    }
    granted = session;
    unsubscribeTheme ??= options.subscribeTheme(() => send({ type: "theme", theme: options.theme() }));
    send({
      type: "init",
      slot: options.slot,
      context: options.context,
      theme: options.theme(),
      plugin: { id: session.plugin, version: session.version },
      scopes: session.scopes,
    });
    options.ui.ready();
  };

  const lacking = (session: HostSession, scopes: readonly (string | null)[]): string | null => {
    const missing = scopes.find((scope) => scope !== null && !session.scopes.includes(scope));
    return missing === undefined ? null : `this plugin has no \`${missing}\` scope`;
  };

  const apiCall = async (session: HostSession, body: Message["body"]): Promise<Answer> => {
    const { method, path } = body;
    if (typeof method !== "string" || typeof path !== "string") return fail("api.call needs a method and a path");
    const found = resolveCall(method, path);
    if (!found.ok) return fail(found.reason);
    const missing = lacking(session, [found.scope]);
    if (missing !== null) return fail(missing);
    const hasBody = body.body !== undefined;
    let res: Response;
    try {
      res = await options.fetch(`${httpBase()}${found.path}`, {
        method: method.toUpperCase(),
        headers: {
          authorization: `Bearer ${session.token}`,
          ...(hasBody ? { "content-type": "application/json" } : {}),
        },
        body: hasBody ? JSON.stringify(body.body) : undefined,
      });
    } catch (e) {
      return fail(`cannot reach the daemon: ${toMessage(e)}`);
    }
    const json: unknown = await res.json().catch(() => undefined);
    if (json === undefined) return fail(`HTTP ${res.status}: response is not JSON`);
    const envelope = parseEnvelope(json);
    return res.ok && envelope.success ? { result: envelope.data } : fail(envelope.error ?? `HTTP ${res.status}`);
  };

  const subscribe = (session: HostSession, body: Message["body"]): Answer => {
    const topics = body.topics;
    if (!Array.isArray(topics) || topics.length === 0 || !topics.every((t) => typeof t === "string")) {
      return fail("topics must be a non-empty list of event names");
    }
    for (const topic of topics as string[]) {
      const needed = eventScopes(topic);
      if (needed === null) return fail(`unknown event topic \`${topic}\``);
      const missing = lacking(session, needed);
      if (missing !== null) return fail(missing);
    }
    const wanted = new Set<string>(topics as string[]);
    events?.close();
    events = options.connectEvents((event) => {
      if (wanted.has(event.type)) send({ type: "event", topic: event.type, event });
    });
    return { result: { topics: [...wanted] } };
  };

  const resize = (body: Message["body"]): Answer => {
    const asked = body.height;
    if (typeof asked !== "number" || !Number.isFinite(asked)) return fail("height must be a number");
    const height = Math.min(maxHeight, Math.max(minHeight, Math.round(asked)));
    options.ui.resize(height);
    return { result: { height } };
  };

  const toast = (body: Message["body"]): Answer => {
    const { text, tone } = body;
    if (typeof text !== "string" || text.trim() === "") return fail("text must be a non-empty string");
    if (!TONES.includes(tone as Tone)) return fail(`tone must be one of ${TONES.join(", ")}`);
    options.ui.toast(text.slice(0, TOAST_MAX_CHARS), tone as Tone);
    return { result: {} };
  };

  const navigateTo = (body: Message["body"]): Answer => {
    if (!townhallRoute(body.path)) return fail("path is not a townhall route");
    options.ui.navigate(body.path);
    return { result: {} };
  };

  const request = async (session: HostSession, message: Message): Promise<Answer> => {
    switch (message.type) {
      case "api.call":
        return apiCall(session, message.body);
      case "events.subscribe":
        return subscribe(session, message.body);
      case "ui.resize":
        return resize(message.body);
      case "ui.toast":
        return toast(message.body);
      case "ui.navigate":
        return navigateTo(message.body);
      default:
        return fail(`unknown message type \`${message.type}\``);
    }
  };

  const dispatch = async (message: Message) => {
    if (message.type === "hello") {
      await hello(message);
      return;
    }
    if (granted === null) {
      reply(message.id, fail("send hello first"));
      return;
    }
    reply(message.id, await request(granted, message));
  };

  return {
    handle: (event) => {
      if (stopped || event.source === null || event.source !== options.frame()) return;
      if (!limiter()) {
        stop();
        options.ui.paused();
        return;
      }
      const message = readMessage(event.data);
      if (message === null) return;
      dispatch(message).catch((e: unknown) => reply(message.id, fail(toMessage(e))));
    },
    dispose: stop,
  };
}
