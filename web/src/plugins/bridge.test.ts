import { describe, expect, it, vi } from "vitest";
import type { ConnectEvents } from "../api/events";
import type { ServerEvent } from "../api/types";
import { createFakeEvents } from "../data/testing";
import { createBridge, type BridgeOptions, type HostSession } from "./bridge";

const TOKEN = "cd".repeat(32);

const session = (scopes: readonly string[]): HostSession => ({
  plugin: "panel@acme",
  version: "1.0.0",
  token: TOKEN,
  scopes,
});

const theme = { resolved: "dark" as const, tokens: { "--th-accent": "#fff" } };

interface Rig {
  readonly bridge: ReturnType<typeof createBridge>;
  readonly frame: { postMessage: ReturnType<typeof vi.fn> };
  readonly fetch: ReturnType<typeof vi.fn>;
  readonly ui: {
    resize: ReturnType<typeof vi.fn>;
    toast: ReturnType<typeof vi.fn>;
    navigate: ReturnType<typeof vi.fn>;
    ready: ReturnType<typeof vi.fn>;
    paused: ReturnType<typeof vi.fn>;
    failed: ReturnType<typeof vi.fn>;
  };
  readonly events: ReturnType<typeof createFakeEvents>;
  readonly say: (data: unknown, source?: unknown) => Promise<void>;
  readonly sent: () => Record<string, unknown>[];
  readonly themeListeners: Set<() => void>;
}

function rig(scopes: readonly string[], extra: Partial<BridgeOptions> = {}, answer: unknown = { success: true, data: { ok: 1 }, error: null }): Rig {
  const frame = { postMessage: vi.fn() };
  const fetchFn = vi.fn(async () => Response.json(answer));
  const ui = {
    resize: vi.fn(),
    toast: vi.fn(),
    navigate: vi.fn(),
    ready: vi.fn(),
    paused: vi.fn(),
    failed: vi.fn(),
  };
  const events = createFakeEvents();
  const themeListeners = new Set<() => void>();
  const bridge = createBridge({
    slot: "session.section",
    context: { conversationId: "c1" },
    frame: () => frame as unknown as Window,
    session: () => Promise.resolve(session(scopes)),
    fetch: fetchFn as unknown as typeof fetch,
    connectEvents: events.connect as ConnectEvents,
    theme: () => theme,
    subscribeTheme: (listener) => {
      themeListeners.add(listener);
      return () => themeListeners.delete(listener);
    },
    ui,
    ...extra,
  });
  const say = async (data: unknown, source: unknown = frame) => {
    bridge.handle({ source, data });
    await vi.waitFor(() => undefined);
    await new Promise((resolve) => setTimeout(resolve, 0));
  };
  const sent = () => frame.postMessage.mock.calls.map((call) => call[0] as Record<string, unknown>);
  return { bridge, frame, fetch: fetchFn, ui, events, say, sent, themeListeners };
}

const hello = { th: 1, type: "hello", apiVersion: 1 };

async function ready(scopes: readonly string[], extra: Partial<BridgeOptions> = {}, answer?: unknown): Promise<Rig> {
  const r = rig(scopes, extra, answer);
  await r.say(hello);
  r.frame.postMessage.mockClear();
  return r;
}

const call = (path: string, id = 1, method = "GET", body?: unknown) => ({
  th: 1,
  id,
  type: "api.call",
  method,
  path,
  ...(body === undefined ? {} : { body }),
});

describe("source", () => {
  it("ignores a message whose source is not the frame, even a hello", async () => {
    const r = rig(["tasks:read"]);
    await r.say(hello, { postMessage: vi.fn() });
    await r.say(hello, null);
    await r.say(call("/api/plugin/v1/tasks"), window);
    expect(r.sent()).toEqual([]);
    expect(r.fetch).not.toHaveBeenCalled();
    expect(r.ui.ready).not.toHaveBeenCalled();
  });

  it("does not count a foreign message against the frame's rate limit", async () => {
    const limiter = vi.fn(() => true);
    const r = rig([], { limiter });
    await r.say(hello, window);
    expect(limiter).not.toHaveBeenCalled();
  });

  it("ignores data that is not a townhall message", async () => {
    const r = rig([]);
    for (const data of ["hello", null, 4, [], {}, { th: 2, type: "hello" }, { th: 1 }, { th: 1, type: 5 }]) {
      await r.say(data);
    }
    expect(r.sent()).toEqual([]);
  });
});

describe("hello", () => {
  it("answers init with the slot, context, theme, plugin and scopes", async () => {
    const r = rig(["tasks:read"]);
    await r.say(hello);
    expect(r.sent()).toEqual([
      {
        th: 1,
        type: "init",
        slot: "session.section",
        context: { conversationId: "c1" },
        theme,
        plugin: { id: "panel@acme", version: "1.0.0" },
        scopes: ["tasks:read"],
      },
    ]);
    expect(r.frame.postMessage).toHaveBeenCalledWith(expect.anything(), "*");
    expect(r.ui.ready).toHaveBeenCalledTimes(1);
  });

  it("never sends the token to the frame", async () => {
    const r = await ready(["tasks:read"]);
    await r.say(call("/api/plugin/v1/tasks"));
    await r.say(call("/api/plugin/v1/info", 2));
    expect(JSON.stringify(r.sent())).not.toContain(TOKEN);
  });

  it("refuses another api version", async () => {
    const r = rig([]);
    await r.say({ th: 1, id: 7, type: "hello", apiVersion: 2 });
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 7, error: "plugin api version 2 is not supported, this host speaks 1" }]);
    expect(r.ui.ready).not.toHaveBeenCalled();
  });

  it("reports a failed token as a failure of the frame", async () => {
    const r = rig([], { session: () => Promise.reject(new Error("no token")) });
    await r.say(hello);
    expect(r.ui.failed).toHaveBeenCalledWith("no token");
    expect(r.sent()).toEqual([]);
  });

  it("refuses requests before hello", async () => {
    const r = rig(["tasks:read"]);
    await r.say(call("/api/plugin/v1/tasks"));
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, error: "send hello first" }]);
    expect(r.fetch).not.toHaveBeenCalled();
  });

  it("sends theme changes after hello", async () => {
    const r = await ready([]);
    for (const listener of r.themeListeners) listener();
    expect(r.sent()).toEqual([{ th: 1, type: "theme", theme }]);
  });
});

describe("api.call", () => {
  it("forwards to the plugin api with the host's token and answers the data", async () => {
    const r = await ready(["tasks:read"]);
    await r.say(call("/api/plugin/v1/tasks?repo=shop"));
    expect(r.fetch).toHaveBeenCalledTimes(1);
    const [url, init] = r.fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/plugin/v1/tasks?repo=shop");
    expect(init.method).toBe("GET");
    expect(init.headers).toEqual({ authorization: `Bearer ${TOKEN}` });
    expect(init.body).toBeUndefined();
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, result: { ok: 1 } }]);
  });

  it("sends a body as json and takes no headers from the frame", async () => {
    const r = await ready(["storage"]);
    await r.say({ ...call("/api/plugin/v1/storage/k", 3, "PUT", { value: "v" }), headers: { authorization: "Bearer x" } });
    const [, init] = r.fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(init.method).toBe("PUT");
    expect(init.headers).toEqual({ authorization: `Bearer ${TOKEN}`, "content-type": "application/json" });
    expect(init.body).toBe('{"value":"v"}');
  });

  it("refuses a path outside /api/plugin/v1/ without a request", async () => {
    const r = await ready(["tasks:read", "settings:read"]);
    await r.say(call("/api/settings"));
    await r.say(call("/api/plugin/v1/../tasks", 2));
    await r.say(call("https://evil.example/api/plugin/v1/tasks", 3));
    expect(r.fetch).not.toHaveBeenCalled();
    const answers = r.sent();
    expect(answers).toHaveLength(3);
    for (const answer of answers) expect(answer).toMatchObject({ type: "response", error: expect.stringContaining("") });
    expect(answers[0]).toMatchObject({ id: 1, error: expect.stringContaining("/api/plugin/v1/") });
  });

  it("refuses a call that needs a scope the plugin lacks without a request", async () => {
    const r = await ready(["tasks:read"]);
    await r.say(call("/api/plugin/v1/conversations", 5));
    await r.say(call("/api/plugin/v1/conversations/c1/messages", 6, "POST", { text: "hi" }));
    expect(r.fetch).not.toHaveBeenCalled();
    expect(r.sent()).toEqual([
      { th: 1, type: "response", id: 5, error: "this plugin has no `conversations:read` scope" },
      { th: 1, type: "response", id: 6, error: "this plugin has no `conversations:write` scope" },
    ]);
  });

  it("needs no scope for info", async () => {
    const r = await ready([]);
    await r.say(call("/api/plugin/v1/info"));
    expect(r.fetch).toHaveBeenCalledTimes(1);
  });

  it("answers the daemon's error message when it refuses", async () => {
    const r = await ready(["tasks:read"], {}, { success: false, data: null, error: "this token has no `tasks:read` scope" });
    await r.say(call("/api/plugin/v1/tasks"));
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, error: "this token has no `tasks:read` scope" }]);
  });

  it("answers an error when the daemon cannot be reached", async () => {
    const r = await ready(["tasks:read"], { fetch: (() => Promise.reject(new Error("offline"))) as unknown as typeof fetch });
    await r.say(call("/api/plugin/v1/tasks"));
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, error: "cannot reach the daemon: offline" }]);
  });

  it("refuses a malformed call", async () => {
    const r = await ready(["tasks:read"]);
    await r.say({ th: 1, id: 1, type: "api.call", method: 5, path: "/api/plugin/v1/tasks" });
    await r.say({ th: 1, id: 2, type: "api.call", method: "GET" });
    expect(r.fetch).not.toHaveBeenCalled();
    expect(r.sent()).toHaveLength(2);
  });
});

const event = (type: "taskUpdated" | "entryAppended"): ServerEvent =>
  type === "taskUpdated"
    ? ({ type, task: { name: "one" } } as unknown as ServerEvent)
    : ({ type, conversationId: "c1", entry: {} } as unknown as ServerEvent);

describe("events.subscribe", () => {
  const subscribe = (topics: unknown, id = 1) => ({ th: 1, id, type: "events.subscribe", topics });

  it("forwards only the subscribed topics the scopes allow", async () => {
    const r = await ready(["events", "tasks:read", "conversations:read"]);
    await r.say(subscribe(["taskUpdated"]));
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, result: { topics: ["taskUpdated"] } }]);
    r.frame.postMessage.mockClear();
    r.events.emit(event("entryAppended"));
    r.events.emit(event("taskUpdated"));
    r.events.emit({ type: "resync" });
    expect(r.sent()).toEqual([{ th: 1, type: "event", topic: "taskUpdated", event: event("taskUpdated") }]);
  });

  it("refuses a topic whose scope the plugin lacks and opens no socket", async () => {
    const r = await ready(["events", "tasks:read"]);
    await r.say(subscribe(["taskUpdated", "entryAppended"]));
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, error: "this plugin has no `conversations:read` scope" }]);
    expect(r.events.isConnected()).toBe(false);
  });

  it("refuses without the events scope", async () => {
    const r = await ready(["tasks:read"]);
    await r.say(subscribe(["taskUpdated"]));
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, error: "this plugin has no `events` scope" }]);
    expect(r.events.isConnected()).toBe(false);
  });

  it("refuses a topic that does not exist and a malformed list", async () => {
    const r = await ready(["events", "tasks:read"]);
    await r.say(subscribe(["resync"]));
    await r.say(subscribe("taskUpdated", 2));
    await r.say(subscribe([], 3));
    expect(r.sent().every((m) => typeof m.error === "string")).toBe(true);
    expect(r.events.isConnected()).toBe(false);
  });

  it("closes the socket when the bridge is disposed", async () => {
    const r = await ready(["events", "tasks:read"]);
    await r.say(subscribe(["taskUpdated"]));
    expect(r.events.isConnected()).toBe(true);
    r.bridge.dispose();
    expect(r.events.isConnected()).toBe(false);
  });

  it("replaces an earlier subscription", async () => {
    const r = await ready(["events", "tasks:read", "conversations:read"]);
    await r.say(subscribe(["taskUpdated"]));
    await r.say(subscribe(["entryAppended"], 2));
    r.frame.postMessage.mockClear();
    r.events.emit(event("taskUpdated"));
    r.events.emit(event("entryAppended"));
    expect(r.sent().map((m) => m.topic)).toEqual(["entryAppended"]);
  });
});

describe("ui", () => {
  it("clamps a resize to the slot's limits and answers the height used", async () => {
    const r = await ready([], { heightRange: [24, 400] });
    await r.say({ th: 1, id: 1, type: "ui.resize", height: 9000 });
    await r.say({ th: 1, id: 2, type: "ui.resize", height: 3 });
    await r.say({ th: 1, id: 3, type: "ui.resize", height: 120.4 });
    expect(r.ui.resize.mock.calls).toEqual([[400], [24], [120]]);
    expect(r.sent()).toEqual([
      { th: 1, type: "response", id: 1, result: { height: 400 } },
      { th: 1, type: "response", id: 2, result: { height: 24 } },
      { th: 1, type: "response", id: 3, result: { height: 120 } },
    ]);
  });

  it("refuses a resize that is not a number", async () => {
    const r = await ready([]);
    await r.say({ th: 1, id: 1, type: "ui.resize", height: "tall" });
    await r.say({ th: 1, id: 2, type: "ui.resize", height: Number.NaN });
    expect(r.ui.resize).not.toHaveBeenCalled();
    expect(r.sent().every((m) => typeof m.error === "string")).toBe(true);
  });

  it("shows a toast of plain text cut to 200 characters", async () => {
    const r = await ready([]);
    await r.say({ th: 1, id: 1, type: "ui.toast", text: "x".repeat(500), tone: "warn" });
    expect(r.ui.toast).toHaveBeenCalledWith("x".repeat(200), "warn");
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, result: {} }]);
  });

  it("refuses a toast with no text or an unknown tone", async () => {
    const r = await ready([]);
    await r.say({ th: 1, id: 1, type: "ui.toast", text: "", tone: "info" });
    await r.say({ th: 1, id: 2, type: "ui.toast", text: "hi", tone: "loud" });
    await r.say({ th: 1, id: 3, type: "ui.toast", text: 4, tone: "info" });
    expect(r.ui.toast).not.toHaveBeenCalled();
    expect(r.sent()).toHaveLength(3);
  });

  it("navigates to a townhall route", async () => {
    const r = await ready([]);
    await r.say({ th: 1, id: 1, type: "ui.navigate", path: "/c/abc" });
    await r.say({ th: 1, id: 2, type: "ui.navigate", path: "/settings" });
    expect(r.ui.navigate.mock.calls).toEqual([["/c/abc"], ["/settings"]]);
  });

  it("refuses a path that is not a townhall route", async () => {
    const r = await ready([]);
    for (const [i, path] of ["/nowhere", "https://evil.example/", "//evil.example/c/a", "javascript:alert(1)", "c/abc", 4].entries()) {
      await r.say({ th: 1, id: i, type: "ui.navigate", path });
    }
    expect(r.ui.navigate).not.toHaveBeenCalled();
    expect(r.sent().every((m) => typeof m.error === "string")).toBe(true);
  });

  it("answers an unknown request type with an error", async () => {
    const r = await ready([]);
    await r.say({ th: 1, id: 1, type: "clipboard.write", text: "x" });
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, error: "unknown message type `clipboard.write`" }]);
  });
});

describe("composer.insert", () => {
  const composerUi = (insert: ReturnType<typeof vi.fn>) =>
    ({ ...rig([]).ui, insert }) as unknown as BridgeOptions["ui"];
  const composerRig = async () => {
    const insert = vi.fn();
    const r = await ready([], { slot: "composer.action", ui: composerUi(insert) });
    return { r, insert };
  };

  it("inserts the text into the draft and answers", async () => {
    const { r, insert } = await composerRig();
    await r.say({ th: 1, id: 1, type: "composer.insert", text: "see PR #4" });
    expect(insert).toHaveBeenCalledWith("see PR #4");
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, result: {} }]);
  });

  it("refuses an empty or non-string text", async () => {
    const { r, insert } = await composerRig();
    await r.say({ th: 1, id: 1, type: "composer.insert", text: "" });
    await r.say({ th: 1, id: 2, type: "composer.insert", text: 4 });
    expect(insert).not.toHaveBeenCalled();
    expect(r.sent().every((m) => typeof m.error === "string")).toBe(true);
  });

  it("is unknown outside the composer slot", async () => {
    const r = await ready([]);
    await r.say({ th: 1, id: 1, type: "composer.insert", text: "x" });
    expect(r.sent()).toEqual([{ th: 1, type: "response", id: 1, error: "unknown message type `composer.insert`" }]);
  });

  it("tells a composer frame its action was fired right after init", async () => {
    const r = rig([], { slot: "composer.action", ui: composerUi(vi.fn()) });
    await r.say(hello);
    expect(r.sent().map((m) => m.type)).toEqual(["init", "composer.action"]);
  });

  it("sends no composer.action to other slots", async () => {
    const r = rig([]);
    await r.say(hello);
    expect(r.sent().map((m) => m.type)).toEqual(["init"]);
  });
});

describe("rate limit", () => {
  it("pauses the frame on the message over the limit and answers nothing after", async () => {
    let allowed = 3;
    const r = rig(["tasks:read"], { limiter: () => allowed-- > 0 });
    await r.say(hello);
    await r.say(call("/api/plugin/v1/tasks", 1));
    await r.say(call("/api/plugin/v1/tasks", 2));
    r.frame.postMessage.mockClear();
    await r.say(call("/api/plugin/v1/tasks", 3));
    await r.say(call("/api/plugin/v1/tasks", 4));
    expect(r.ui.paused).toHaveBeenCalledTimes(1);
    expect(r.fetch).toHaveBeenCalledTimes(2);
    expect(r.sent()).toEqual([]);
  });

  it("trips with the real limiter at 50 messages in a second", async () => {
    const r = await ready([]);
    for (let i = 0; i < 60; i += 1) r.bridge.handle({ source: r.frame, data: { th: 1, type: "ui.resize", height: 50 } });
    expect(r.ui.paused).toHaveBeenCalledTimes(1);
    expect(r.ui.resize.mock.calls.length).toBe(49);
  });

  it("stops talking to a frame once it is disposed", async () => {
    const r = await ready(["tasks:read"]);
    r.bridge.dispose();
    await r.say(call("/api/plugin/v1/tasks"));
    expect(r.fetch).not.toHaveBeenCalled();
    expect(r.sent()).toEqual([]);
  });
});
