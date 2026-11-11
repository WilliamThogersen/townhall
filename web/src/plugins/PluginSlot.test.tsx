import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConnectEvents } from "../api/events";
import type { PluginUiEntry } from "../api/types";
import { createFakeEvents } from "../data/testing";
import { ToastProvider } from "../ui/Toasts";
import type { HostSession } from "./bridge";
import { forgetHostSession } from "./hostSession";
import { PluginSlot, type SlotServices } from "./PluginSlot";

const TOKEN = "ef".repeat(32);

const entry = (over: Partial<PluginUiEntry> = {}): PluginUiEntry => ({
  plugin: "panel@acme",
  slot: "session.section",
  title: "Panel",
  entry: "ui/panel.html",
  height: 80,
  ...over,
});

interface Harness {
  readonly services: Partial<SlotServices>;
  readonly session: ReturnType<typeof vi.fn>;
  readonly fetch: ReturnType<typeof vi.fn>;
  readonly navigate: ReturnType<typeof vi.fn>;
  readonly events: ReturnType<typeof createFakeEvents>;
}

function harness(entries: readonly PluginUiEntry[], scopes: readonly string[] = ["tasks:read"]): Harness {
  const session = vi.fn(
    async (id: string): Promise<HostSession> => ({ plugin: id, version: "1.0.0", token: TOKEN, scopes }),
  );
  const fetchFn = vi.fn(async () => Response.json({ success: true, data: [{ name: "one" }], error: null }));
  const navigate = vi.fn();
  const events = createFakeEvents();
  return {
    session,
    fetch: fetchFn,
    navigate,
    events,
    services: {
      listUi: async () => entries,
      session,
      fetch: fetchFn as unknown as typeof fetch,
      connectEvents: events.connect as ConnectEvents,
      navigate,
    },
  };
}

function mount(h: Harness, props: { name?: string; context?: Record<string, unknown> } = {}) {
  return render(
    <ToastProvider>
      <div data-testid="slot">
        <PluginSlot name={props.name ?? "session.section"} context={props.context ?? { conversationId: "c1" }} services={h.services} />
      </div>
    </ToastProvider>,
  );
}

async function frameOf(container: HTMLElement): Promise<HTMLIFrameElement> {
  await waitFor(() => expect(container.querySelector("iframe")).not.toBeNull());
  return container.querySelector("iframe") as HTMLIFrameElement;
}

function post(frame: HTMLIFrameElement, data: unknown, source: unknown = frame.contentWindow) {
  act(() => {
    window.dispatchEvent(new MessageEvent("message", { data, source: source as MessageEventSource }));
  });
}

const settle = () => act(() => new Promise<void>((resolve) => setTimeout(resolve, 0)));

const hello = { th: 1, type: "hello", apiVersion: 1 };

async function readyFrame(h: Harness) {
  const view = mount(h);
  const frame = await frameOf(view.container);
  const spy = vi.spyOn(frame.contentWindow as Window, "postMessage");
  post(frame, hello);
  await waitFor(() => expect(spy).toHaveBeenCalled());
  spy.mockClear();
  return { ...view, frame, spy };
}

const lastAnswer = (spy: ReturnType<typeof vi.spyOn>) => spy.mock.calls.at(-1)?.[0] as Record<string, unknown>;

beforeEach(() => forgetHostSession("panel@acme"));
afterEach(() => vi.useRealTimers());

describe("PluginSlot", () => {
  it("renders nothing with no plugin", async () => {
    const h = harness([]);
    mount(h);
    await settle();
    expect(screen.getByTestId("slot").innerHTML).toBe("");
  });

  it("renders nothing while the list is loading and when no entry is for this slot", async () => {
    const h = harness([entry({ slot: "page" })]);
    mount(h);
    expect(screen.getByTestId("slot").innerHTML).toBe("");
    await settle();
    expect(screen.getByTestId("slot").innerHTML).toBe("");
    expect(h.session).not.toHaveBeenCalled();
  });

  it("renders nothing and keeps the app working when the list cannot be read", async () => {
    const h = harness([]);
    const view = render(
      <ToastProvider>
        <PluginSlot name="page" services={{ ...h.services, listUi: () => Promise.reject(new Error("daemon down")) }} />
      </ToastProvider>,
    );
    await settle();
    expect(view.container.querySelector("iframe")).toBeNull();
  });

  it("renders the sandboxed frame of a plugin entry with its height", async () => {
    const view = mount(harness([entry(), entry({ plugin: "other@acme" })]));
    const frame = await frameOf(view.container);
    expect(view.container.querySelectorAll("iframe")).toHaveLength(2);
    expect(frame.getAttribute("sandbox")).toBe("allow-scripts");
    expect(frame.getAttribute("src")).toBe("/api/plugins/panel%40acme/ui/ui/panel.html");
    expect(frame.getAttribute("title")).toBe("Panel");
    expect(frame.style.height).toBe("80px");
  });

  it("ignores a message whose source is not the frame", async () => {
    const h = harness([entry()]);
    const view = mount(h);
    const frame = await frameOf(view.container);
    const spy = vi.spyOn(frame.contentWindow as Window, "postMessage");
    post(frame, hello, window);
    post(frame, hello, null);
    post(frame, hello, document.createElement("iframe").contentWindow ?? window);
    await settle();
    expect(spy).not.toHaveBeenCalled();
    expect(h.session).not.toHaveBeenCalled();
  });

  it("sends init with slot, context, plugin and scopes, never the token", async () => {
    const h = harness([entry()]);
    const view = mount(h, { context: { conversationId: "c9" } });
    const frame = await frameOf(view.container);
    const spy = vi.spyOn(frame.contentWindow as Window, "postMessage");
    post(frame, hello);
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const init = lastAnswer(spy);
    expect(init).toMatchObject({
      th: 1,
      type: "init",
      slot: "session.section",
      context: { conversationId: "c9" },
      plugin: { id: "panel@acme", version: "1.0.0" },
      scopes: ["tasks:read"],
    });
    expect(JSON.stringify(spy.mock.calls)).not.toContain(TOKEN);
    expect(spy.mock.calls[0]?.[1]).toBe("*");
  });

  it("forwards an allowed api.call with the host's token", async () => {
    const h = harness([entry()]);
    const { spy } = await readyFrame(h);
    const frame = document.querySelector("iframe") as HTMLIFrameElement;
    post(frame, { th: 1, id: 1, type: "api.call", method: "GET", path: "/api/plugin/v1/tasks" });
    await waitFor(() => expect(spy).toHaveBeenCalled());
    const [url, init] = h.fetch.mock.calls[0] as unknown as [string, RequestInit];
    expect(url).toBe("/api/plugin/v1/tasks");
    expect(init.headers).toEqual({ authorization: `Bearer ${TOKEN}` });
    expect(lastAnswer(spy)).toEqual({ th: 1, type: "response", id: 1, result: [{ name: "one" }] });
    expect(JSON.stringify(spy.mock.calls)).not.toContain(TOKEN);
  });

  it("refuses a path outside /api/plugin/v1/", async () => {
    const h = harness([entry()], ["tasks:read", "settings:read"]);
    const { spy, frame } = await readyFrame(h);
    post(frame, { th: 1, id: 2, type: "api.call", method: "GET", path: "/api/settings" });
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(h.fetch).not.toHaveBeenCalled();
    expect(lastAnswer(spy)).toMatchObject({ type: "response", id: 2, error: expect.stringContaining("/api/plugin/v1/") });
  });

  it("refuses a call that needs a scope the plugin lacks", async () => {
    const h = harness([entry()], ["tasks:read"]);
    const { spy, frame } = await readyFrame(h);
    post(frame, { th: 1, id: 3, type: "api.call", method: "GET", path: "/api/plugin/v1/conversations" });
    await waitFor(() => expect(spy).toHaveBeenCalled());
    expect(h.fetch).not.toHaveBeenCalled();
    expect(lastAnswer(spy)).toEqual({
      th: 1,
      type: "response",
      id: 3,
      error: "this plugin has no `conversations:read` scope",
    });
  });

  it("forwards the events the scopes allow", async () => {
    const h = harness([entry()], ["events", "tasks:read"]);
    const { spy, frame } = await readyFrame(h);
    post(frame, { th: 1, id: 4, type: "events.subscribe", topics: ["taskUpdated"] });
    await waitFor(() => expect(spy).toHaveBeenCalled());
    spy.mockClear();
    act(() => h.events.emit({ type: "taskUpdated", task: { name: "one" } } as never));
    expect(lastAnswer(spy)).toMatchObject({ type: "event", topic: "taskUpdated" });
  });

  it("resizes the frame within the slot's limits", async () => {
    const h = harness([entry()]);
    const { spy, frame } = await readyFrame(h);
    post(frame, { th: 1, id: 5, type: "ui.resize", height: 5000 });
    await waitFor(() => expect(frame.style.height).toBe("400px"));
    expect(lastAnswer(spy)).toMatchObject({ id: 5, result: { height: 400 } });
  });

  it("limits sidebar.foot to 120 px", async () => {
    const h = harness([entry({ slot: "sidebar.foot", height: 40 })]);
    const view = render(
      <ToastProvider>
        <PluginSlot name="sidebar.foot" services={h.services} />
      </ToastProvider>,
    );
    const frame = await frameOf(view.container);
    post(frame, hello);
    await settle();
    post(frame, { th: 1, id: 1, type: "ui.resize", height: 900 });
    await waitFor(() => expect(frame.style.height).toBe("120px"));
  });

  it("shows a toast with the plugin's name and navigates to a townhall route", async () => {
    const h = harness([entry()]);
    const { frame } = await readyFrame(h);
    post(frame, { th: 1, id: 6, type: "ui.toast", text: "saved it", tone: "success" });
    expect(await screen.findByText(/saved it/)).toBeInTheDocument();
    expect(screen.getByText("panel@acme")).toBeInTheDocument();
    post(frame, { th: 1, id: 7, type: "ui.navigate", path: "/settings" });
    await waitFor(() => expect(h.navigate).toHaveBeenCalledWith("/settings"));
  });

  it("pauses a flooding frame, shows a notice and removes the frame", async () => {
    const h = harness([entry()]);
    const { frame, container } = await readyFrame(h);
    for (let i = 0; i < 60; i += 1) post(frame, { th: 1, type: "ui.resize", height: 100 });
    expect(await screen.findByRole("alert")).toHaveTextContent("plugin panel@acme was paused");
    expect(container.querySelector("iframe")).toBeNull();
  });

  it("resumes a paused frame with a fresh frame on request", async () => {
    const h = harness([entry()]);
    const { frame, container } = await readyFrame(h);
    for (let i = 0; i < 60; i += 1) post(frame, { th: 1, type: "ui.resize", height: 100 });
    fireEvent.click(await screen.findByRole("button", { name: "Resume" }));
    expect(await frameOf(container)).not.toBe(frame);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("replaces a frame that never says hello within 5 s with a failure line and a retry", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const h = harness([entry()]);
    const view = mount(h);
    await frameOf(view.container);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(5001);
    });
    expect(screen.getByRole("alert")).toHaveTextContent("plugin panel@acme failed to load: no hello within 5 s");
    expect(view.container.querySelector("iframe")).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    await frameOf(view.container);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("does not time out a frame that said hello", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    const h = harness([entry()]);
    const view = mount(h);
    const frame = await frameOf(view.container);
    post(frame, hello);
    await act(async () => {
      await vi.advanceTimersByTimeAsync(6000);
    });
    expect(screen.queryByRole("alert")).toBeNull();
    expect(view.container.querySelector("iframe")).not.toBeNull();
  });

  it("shows a failure line when no token can be had, and retries with a new token request", async () => {
    const h = harness([entry()]);
    h.session.mockRejectedValueOnce(new Error("enabled plugin `panel@acme` not found"));
    const view = mount(h);
    const frame = await frameOf(view.container);
    post(frame, hello);
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "plugin panel@acme failed to load: enabled plugin `panel@acme` not found",
    );
    fireEvent.click(screen.getByRole("button", { name: "Retry" }));
    const again = await frameOf(view.container);
    post(again, hello);
    await waitFor(() => expect(h.session).toHaveBeenCalledTimes(2));
  });

  it("stops listening when it unmounts", async () => {
    const h = harness([entry()]);
    const { frame, unmount } = await readyFrame(h);
    unmount();
    post(frame, { th: 1, id: 1, type: "api.call", method: "GET", path: "/api/plugin/v1/tasks" });
    await settle();
    expect(h.fetch).not.toHaveBeenCalled();
  });
});
