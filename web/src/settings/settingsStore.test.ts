import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import type { PutSetting } from "./browserImport";
import { DEFAULT_SETTINGS, type Settings } from "./settings";
import { createSettingsStore, type SettingsStoreDeps } from "./settingsStore";

interface Deferred<T> {
  readonly promise: Promise<T>;
  readonly resolve: (value: T) => void;
  readonly reject: (reason: unknown) => void;
}

function deferred<T>(): Deferred<T> {
  let resolve: (value: T) => void = () => undefined;
  let reject: (reason: unknown) => void = () => undefined;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const flush = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
const down = () => new ApiError("network", "cannot reach the daemon: boom");

function fakeDeps(initial: Partial<Settings> = {}) {
  let daemon: Settings = { ...DEFAULT_SETTINGS, ...initial };
  const load = vi.fn(async () => daemon);
  const put = vi.fn<PutSetting>(async (key, value) => {
    daemon = { ...daemon, [key]: value };
    return daemon;
  });
  const deps: SettingsStoreDeps = { load, put, storage: () => localStorage };
  return { deps, load, put, daemon: () => daemon };
}

async function loadedStore(initial: Partial<Settings> = {}) {
  const fake = fakeDeps(initial);
  const store = createSettingsStore(fake.deps);
  store.ensureLoaded();
  await flush();
  return { ...fake, store };
}

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("loading", () => {
  it("shows the defaults until it is loaded", () => {
    const { deps, load } = fakeDeps({ agentsAtOnce: 9 });
    const store = createSettingsStore(deps);
    expect(store.getState()).toEqual({ settings: DEFAULT_SETTINGS, loading: false, problem: null });
    expect(load).not.toHaveBeenCalled();
    store.ensureLoaded();
    expect(store.getState()).toEqual({ settings: DEFAULT_SETTINGS, loading: true, problem: null });
  });

  it("shows the daemon's settings once loaded", async () => {
    const { store } = await loadedStore({ agentsAtOnce: 9, permissionMode: "plan" });
    expect(store.getState().loading).toBe(false);
    expect(store.getState().settings).toEqual({ ...DEFAULT_SETTINGS, agentsAtOnce: 9, permissionMode: "plan" });
  });

  it("loads once however often it is asked", async () => {
    const { store, load } = await loadedStore();
    store.ensureLoaded();
    store.ensureLoaded();
    await flush();
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("returns the same state until something changes", async () => {
    const { store } = await loadedStore();
    const first = store.getState();
    expect(store.getState()).toBe(first);
    store.update({ agentsAtOnce: 7 });
    expect(store.getState()).not.toBe(first);
    expect(first.settings.agentsAtOnce).toBe(6);
  });

  it("keeps the defaults and reports a failed load, then retries", async () => {
    const { deps, load } = fakeDeps({ agentsAtOnce: 9 });
    load.mockRejectedValueOnce(down());
    const store = createSettingsStore(deps);
    store.ensureLoaded();
    await flush();
    expect(store.getState()).toEqual({
      settings: DEFAULT_SETTINGS,
      loading: false,
      problem: { kind: "load", message: "settings not loaded: cannot reach the daemon: boom" },
    });
    store.reload();
    expect(store.getState().loading).toBe(true);
    await flush();
    expect(load).toHaveBeenCalledTimes(2);
    expect(store.getState()).toEqual({ settings: { ...DEFAULT_SETTINGS, agentsAtOnce: 9 }, loading: false, problem: null });
  });

  it("retries a failed load when asked to ensure it again", async () => {
    const { deps, load } = fakeDeps();
    load.mockRejectedValueOnce(down());
    const store = createSettingsStore(deps);
    store.ensureLoaded();
    await flush();
    store.ensureLoaded();
    await flush();
    expect(load).toHaveBeenCalledTimes(2);
    expect(store.getState().problem).toBeNull();
  });

  it("does not start a second load while one runs", async () => {
    const { deps, load } = fakeDeps();
    load.mockReturnValueOnce(deferred<Settings>().promise);
    const store = createSettingsStore(deps);
    store.ensureLoaded();
    store.reload();
    store.ensureLoaded();
    await flush();
    expect(load).toHaveBeenCalledTimes(1);
    expect(store.getState().loading).toBe(true);
  });
});

describe("changes", () => {
  it("shows a change at once and saves it with one PUT", async () => {
    const { store, put, daemon } = await loadedStore();
    store.update({ repoUnclear: "guess" });
    expect(store.getState().settings.repoUnclear).toBe("guess");
    await flush();
    expect(put.mock.calls).toEqual([["repoUnclear", "guess"]]);
    expect(daemon().repoUnclear).toBe("guess");
    expect(store.getState().settings.repoUnclear).toBe("guess");
  });

  it("sends one PUT per changed key and skips the rest", async () => {
    const { store, put } = await loadedStore();
    store.update({ permissionMode: "bypassPermissions", workWithoutAsking: true, agentsAtOnce: 6 });
    await flush();
    expect(put.mock.calls).toEqual([
      ["permissionMode", "bypassPermissions"],
      ["workWithoutAsking", true],
    ]);
  });

  it("skips saving and subscribers when nothing changes", async () => {
    const { store, put } = await loadedStore();
    const listener = vi.fn();
    store.subscribe(listener);
    store.update({ agentsAtOnce: 6, notifyWhen: "needsYou" });
    await flush();
    expect(put).not.toHaveBeenCalled();
    expect(listener).not.toHaveBeenCalled();
  });

  it("keeps agents at once inside its limits", async () => {
    const { store, put } = await loadedStore();
    store.update({ agentsAtOnce: 80 });
    expect(store.getState().settings.agentsAtOnce).toBe(50);
    store.update({ agentsAtOnce: 0 });
    expect(store.getState().settings.agentsAtOnce).toBe(1);
    await flush();
    expect(put.mock.calls).toEqual([
      ["agentsAtOnce", 50],
      ["agentsAtOnce", 1],
    ]);
  });

  it("ignores values that are not valid", async () => {
    const { store, put } = await loadedStore();
    const bad = JSON.parse('{"permissionMode":"yolo","agentsAtOnce":"many"}') as Partial<Settings>;
    store.update(bad);
    await flush();
    expect(put).not.toHaveBeenCalled();
    expect(store.getState().settings).toEqual(DEFAULT_SETTINGS);
  });

  it("rolls a failed write back and says why", async () => {
    const { store, put } = await loadedStore({ agentsAtOnce: 4 });
    put.mockRejectedValueOnce(down());
    store.update({ agentsAtOnce: 5 });
    expect(store.getState().settings.agentsAtOnce).toBe(5);
    await flush();
    expect(store.getState().settings.agentsAtOnce).toBe(4);
    expect(store.getState().problem).toEqual({
      kind: "save",
      message: "settings not saved: cannot reach the daemon: boom",
    });
  });

  it("clears a save problem on the next good save, or when dismissed", async () => {
    const { store, put } = await loadedStore();
    put.mockRejectedValueOnce(down());
    store.update({ notifyWhen: "failed" });
    await flush();
    store.update({ notifyWhen: "everything" });
    await flush();
    expect(store.getState().problem).toBeNull();
    expect(store.getState().settings.notifyWhen).toBe("everything");

    put.mockRejectedValueOnce(down());
    store.update({ notifyWhen: "failed" });
    await flush();
    expect(store.getState().problem?.kind).toBe("save");
    store.dismissProblem();
    expect(store.getState().problem).toBeNull();
    const listener = vi.fn();
    store.subscribe(listener);
    store.dismissProblem();
    expect(listener).not.toHaveBeenCalled();
  });

  it("writes one at a time, in order", async () => {
    const { store, put } = await loadedStore();
    const first = deferred<Settings>();
    put.mockReturnValueOnce(first.promise);
    store.update({ agentsAtOnce: 7 });
    store.update({ agentsAtOnce: 8 });
    await flush();
    expect(put).toHaveBeenCalledTimes(1);
    expect(store.getState().settings.agentsAtOnce).toBe(8);
    first.resolve({ ...DEFAULT_SETTINGS, agentsAtOnce: 7 });
    await flush();
    expect(put.mock.calls).toEqual([
      ["agentsAtOnce", 7],
      ["agentsAtOnce", 8],
    ]);
    expect(store.getState().settings.agentsAtOnce).toBe(8);
  });

  it("keeps a later change when an earlier one fails", async () => {
    const { store, put } = await loadedStore();
    put.mockRejectedValueOnce(down());
    store.update({ agentsAtOnce: 7 });
    store.update({ agentsAtOnce: 8 });
    await flush();
    expect(store.getState().settings.agentsAtOnce).toBe(8);
  });

  it("waits for the load before writing and keeps the change on top of it", async () => {
    const { deps, load, put } = fakeDeps({ agentsAtOnce: 9, repoUnclear: "guess" });
    const loading = deferred<Settings>();
    load.mockReturnValueOnce(loading.promise);
    const store = createSettingsStore(deps);
    store.ensureLoaded();
    store.update({ notifyWhen: "everything" });
    await flush();
    expect(put).not.toHaveBeenCalled();
    loading.resolve({ ...DEFAULT_SETTINGS, agentsAtOnce: 9, repoUnclear: "guess" });
    await flush();
    expect(put.mock.calls).toEqual([["notifyWhen", "everything"]]);
    expect(store.getState().settings).toEqual({
      ...DEFAULT_SETTINGS,
      agentsAtOnce: 9,
      repoUnclear: "guess",
      notifyWhen: "everything",
    });
  });

  it("tells subscribers about each change until they leave", async () => {
    const { store } = await loadedStore();
    const first = vi.fn();
    const second = vi.fn();
    const leave = store.subscribe(first);
    store.subscribe(second);
    store.update({ agentsAtOnce: 8 });
    expect(first).toHaveBeenCalledTimes(1);
    leave();
    store.update({ agentsAtOnce: 9 });
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(2);
  });
});

describe("browser settings import", () => {
  it("imports th-settings after the first load and clears it", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ ...DEFAULT_SETTINGS, agentsAtOnce: 9 }));
    const { store, put } = await loadedStore();
    expect(put.mock.calls).toEqual([["agentsAtOnce", 9]]);
    expect(store.getState().settings.agentsAtOnce).toBe(9);
    expect(store.getState().problem).toBeNull();
    expect(localStorage.getItem("th-settings")).toBeNull();
  });

  it("keeps th-settings and says so when a value is not imported", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ agentsAtOnce: 9 }));
    const { deps, put } = fakeDeps();
    put.mockRejectedValueOnce(down());
    const store = createSettingsStore(deps);
    store.ensureLoaded();
    await flush();
    expect(store.getState().settings.agentsAtOnce).toBe(6);
    expect(store.getState().problem).toEqual({
      kind: "import",
      message: "settings from this browser not imported: cannot reach the daemon: boom. townhall tries again next time",
    });
    expect(JSON.parse(localStorage.getItem("th-settings") ?? "null")).toEqual({ agentsAtOnce: 9 });
  });

  it("does not import when the load fails", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ agentsAtOnce: 9 }));
    const { deps, load, put } = fakeDeps();
    load.mockRejectedValueOnce(down());
    const store = createSettingsStore(deps);
    store.ensureLoaded();
    await flush();
    expect(put).not.toHaveBeenCalled();
    expect(localStorage.getItem("th-settings")).not.toBeNull();
  });

  it("drops a key from a kept import once you choose it here", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ permissionMode: "bypassPermissions", agentsAtOnce: 9 }));
    const { deps, put } = fakeDeps();
    put.mockRejectedValueOnce(down()).mockRejectedValueOnce(down());
    const store = createSettingsStore(deps);
    store.ensureLoaded();
    await flush();
    store.update({ permissionMode: "default" });
    await flush();
    expect(JSON.parse(localStorage.getItem("th-settings") ?? "null")).toEqual({ agentsAtOnce: 9 });
  });

  it("loads and saves when browser storage throws", async () => {
    const { deps, put } = fakeDeps({ agentsAtOnce: 3 });
    const storage = () => {
      throw new Error("storage denied");
    };
    const store = createSettingsStore({ ...deps, storage });
    store.ensureLoaded();
    await flush();
    store.update({ agentsAtOnce: 4 });
    await flush();
    expect(put.mock.calls).toEqual([["agentsAtOnce", 4]]);
    expect(store.getState().settings.agentsAtOnce).toBe(4);
    expect(console.warn).toHaveBeenCalledWith("warn: browser storage is not available", expect.any(Error));
  });

  it("loads without browser storage", async () => {
    const { deps } = fakeDeps({ agentsAtOnce: 3 });
    const store = createSettingsStore({ ...deps, storage: () => null });
    store.ensureLoaded();
    await flush();
    expect(store.getState().settings.agentsAtOnce).toBe(3);
  });
});

describe("the daemon store", () => {
  it("loads and saves through /api/settings by default", async () => {
    const answer = { ...DEFAULT_SETTINGS, agentsAtOnce: 2 };
    const fetch = vi.fn(async (_url: string, _init?: RequestInit) =>
      Response.json({ success: true, data: answer, error: null }),
    );
    vi.stubGlobal("fetch", fetch);
    const store = createSettingsStore();
    store.ensureLoaded();
    await flush();
    expect(store.getState().settings.agentsAtOnce).toBe(2);
    store.update({ sessionPanelOpen: false });
    await flush();
    expect(fetch.mock.calls.map(([url]) => url)).toEqual(["/api/settings", "/api/settings/sessionPanelOpen"]);
  });
});
