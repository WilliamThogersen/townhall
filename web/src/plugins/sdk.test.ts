import source from "./sdk/townhall-plugin-sdk.js?raw";

interface Sdk {
  readonly ready: Promise<Record<string, unknown>>;
  readonly api: {
    readonly call: (method: string, path: string, body?: unknown) => Promise<unknown>;
  };
  readonly events: {
    readonly subscribe: (topics: string[], handler: (event: unknown) => void) => Promise<unknown>;
  };
  readonly ui: {
    readonly resize: (height: number) => Promise<unknown>;
    readonly toast: (text: string, tone?: string) => Promise<unknown>;
    readonly navigate: (path: string) => Promise<unknown>;
  };
  readonly composer: {
    readonly insert: (text: string) => Promise<unknown>;
    readonly onAction: (handler: () => void) => void;
  };
  readonly theme: { readonly onChange: (handler: (theme: unknown) => void) => void };
}

type Listener = (event: { source: unknown; data: unknown }) => void;

interface Harness {
  readonly sdk: Sdk;
  readonly sent: Record<string, unknown>[];
  readonly parent: object;
  readonly deliver: (data: unknown, source?: unknown) => void;
}

function load(): Harness {
  const sent: Record<string, unknown>[] = [];
  const listeners: Listener[] = [];
  const parent = {
    postMessage: (message: Record<string, unknown>, target: string) => {
      expect(target).toBe("*");
      sent.push(message);
    },
  };
  const fake: Record<string, unknown> = {
    parent,
    addEventListener: (type: string, listener: Listener) => {
      expect(type).toBe("message");
      listeners.push(listener);
    },
  };
  new Function("window", source)(fake);
  const sdk = fake.townhall as Sdk;
  const deliver = (data: unknown, from: unknown = parent) => listeners.forEach((l) => l({ source: from, data }));
  return { sdk, sent, parent, deliver };
}

const INIT = {
  th: 1,
  type: "init",
  slot: "session.section",
  context: { conversationId: "c1" },
  theme: { resolved: "dark", tokens: {} },
  plugin: { id: "notes@acme", version: "1.0.0" },
  scopes: ["tasks:read"],
};

describe("the plugin SDK script", () => {
  it("says hello with the api version as soon as it loads and exposes the init message", async () => {
    const { sdk, sent, deliver } = load();
    expect(sent).toEqual([{ th: 1, type: "hello", apiVersion: 1 }]);
    deliver(INIT);
    await expect(sdk.ready).resolves.toMatchObject({ slot: "session.section", scopes: ["tasks:read"] });
  });

  it("ignores messages that do not come from the host window", async () => {
    const { sdk, deliver } = load();
    deliver({ ...INIT, slot: "evil" }, {});
    deliver(INIT);
    await expect(sdk.ready).resolves.toMatchObject({ slot: "session.section" });
  });

  it("api.call sends the request with an id and resolves with the result", async () => {
    const { sdk, sent, deliver } = load();
    const pending = sdk.api.call("GET", "/api/plugin/v1/tasks");
    const request = sent[1] as { id: number };
    expect(sent[1]).toEqual({
      th: 1,
      id: request.id,
      type: "api.call",
      method: "GET",
      path: "/api/plugin/v1/tasks",
    });
    deliver({ th: 1, type: "response", id: request.id, result: [{ name: "t1" }] });
    await expect(pending).resolves.toEqual([{ name: "t1" }]);
  });

  it("api.call passes a body and rejects with the host's error text", async () => {
    const { sdk, sent, deliver } = load();
    const pending = sdk.api.call("PUT", "/api/plugin/v1/storage/k", { value: "v" });
    const request = sent[1] as { id: number; body: unknown };
    expect(request.body).toEqual({ value: "v" });
    deliver({ th: 1, type: "response", id: request.id, error: "this plugin has no `storage` scope" });
    await expect(pending).rejects.toThrow("this plugin has no `storage` scope");
  });

  it("answers match their own request when several are open", async () => {
    const { sdk, sent, deliver } = load();
    const one = sdk.api.call("GET", "/api/plugin/v1/info");
    const two = sdk.api.call("GET", "/api/plugin/v1/tasks");
    const [, first, second] = sent as { id: number }[];
    deliver({ th: 1, type: "response", id: second?.id, result: "two" });
    deliver({ th: 1, type: "response", id: first?.id, result: "one" });
    await expect(one).resolves.toBe("one");
    await expect(two).resolves.toBe("two");
  });

  it("events.subscribe sends the topics and hands each matching event to the handler", async () => {
    const { sdk, sent, deliver } = load();
    const seen: unknown[] = [];
    const pending = sdk.events.subscribe(["taskUpdated"], (event) => seen.push(event));
    const request = sent[1] as { id: number };
    expect(sent[1]).toMatchObject({ type: "events.subscribe", topics: ["taskUpdated"] });
    deliver({ th: 1, type: "response", id: request.id, result: { topics: ["taskUpdated"] } });
    await pending;
    deliver({ th: 1, type: "event", topic: "taskUpdated", event: { type: "taskUpdated", name: "t1" } });
    deliver({ th: 1, type: "event", topic: "entryAppended", event: { type: "entryAppended" } });
    expect(seen).toEqual([{ type: "taskUpdated", name: "t1" }]);
  });

  it("ui and composer calls send their message types", () => {
    const { sdk, sent } = load();
    void sdk.ui.resize(180);
    void sdk.ui.toast("saved", "success");
    void sdk.ui.toast("plain");
    void sdk.ui.navigate("/settings");
    void sdk.composer.insert("hello");
    const types = sent.slice(1).map(({ id: _id, ...rest }) => rest);
    expect(types).toEqual([
      { th: 1, type: "ui.resize", height: 180 },
      { th: 1, type: "ui.toast", text: "saved", tone: "success" },
      { th: 1, type: "ui.toast", text: "plain", tone: "info" },
      { th: 1, type: "ui.navigate", path: "/settings" },
      { th: 1, type: "composer.insert", text: "hello" },
    ]);
  });

  it("calls the theme and composer.action handlers", () => {
    const { sdk, deliver } = load();
    const themes: unknown[] = [];
    let clicks = 0;
    sdk.theme.onChange((theme) => themes.push(theme));
    sdk.composer.onAction(() => (clicks += 1));
    deliver({ th: 1, type: "theme", theme: { resolved: "light", tokens: {} } });
    deliver({ th: 1, type: "composer.action" });
    expect(themes).toEqual([{ resolved: "light", tokens: {} }]);
    expect(clicks).toBe(1);
  });
});
