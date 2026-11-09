import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import {
  BROWSER_SETTINGS_KEY,
  forgetStoredSetting,
  importBrowserSettings,
  readStoredSettings,
  type PutSetting,
} from "./browserImport";
import { DEFAULT_SETTINGS, type SettingKey, type Settings } from "./settings";

function fakePut(failing: readonly SettingKey[] = []) {
  let current: Settings = DEFAULT_SETTINGS;
  const put = vi.fn<PutSetting>(async (key, value) => {
    if (failing.includes(key)) throw new ApiError("network", "cannot reach the daemon: boom");
    current = { ...current, [key]: value };
    return current;
  });
  return put;
}

const store = (value: unknown) =>
  localStorage.setItem(BROWSER_SETTINGS_KEY, typeof value === "string" ? value : JSON.stringify(value));
const stored = () => localStorage.getItem(BROWSER_SETTINGS_KEY);

function deps(put: PutSetting, storage: Storage | null = localStorage) {
  return { storage, put, onSaved: vi.fn<(settings: Settings) => void>() };
}

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});

afterEach(() => vi.restoreAllMocks());

describe("readStoredSettings", () => {
  it("keeps the known keys with valid values, in key order", () => {
    const raw = JSON.stringify({ notifyWhen: "everything", agentsAtOnce: 9, theme: "light" });
    expect(readStoredSettings(raw)).toEqual([
      { key: "agentsAtOnce", value: 9 },
      { key: "notifyWhen", value: "everything" },
    ]);
  });

  it("drops invalid values", () => {
    const raw = JSON.stringify({ agentsAtOnce: 99, notifyWhen: "never" });
    expect(readStoredSettings(raw)).toEqual([]);
  });

  it("never reads the removed permission settings", () => {
    const raw = JSON.stringify({ permissionMode: "bypassPermissions", workWithoutAsking: true, agentsAtOnce: 9 });
    expect(readStoredSettings(raw)).toEqual([{ key: "agentsAtOnce", value: 9 }]);
  });

  it("never reads forge accounts, which a browser never held", () => {
    const account = { forge: "github", login: "me", host: "github.com", label: "me" };
    expect(readStoredSettings(JSON.stringify({ forgeAccounts: [account], agentsAtOnce: 9 }))).toEqual([
      { key: "agentsAtOnce", value: 9 },
    ]);
  });

  it.each(["{not json", "null", "[1,2]", "\"plan\""])("reads nothing from %s", (raw) => {
    expect(readStoredSettings(raw)).toEqual([]);
  });
});

describe("importBrowserSettings", () => {
  it("does nothing when nothing is stored", async () => {
    const put = fakePut();
    const outcome = await importBrowserSettings(DEFAULT_SETTINGS, deps(put));
    expect(put).not.toHaveBeenCalled();
    expect(outcome).toEqual({ sent: 0, failed: 0, message: null });
    expect(stored()).toBeNull();
  });

  it("does nothing without storage", async () => {
    const put = fakePut();
    const outcome = await importBrowserSettings(DEFAULT_SETTINGS, deps(put, null));
    expect(put).not.toHaveBeenCalled();
    expect(outcome).toEqual({ sent: 0, failed: 0, message: null });
  });

  it("sends each stored value that differs from the daemon, then clears th-settings", async () => {
    store({
      agentsAtOnce: 9,
      repoUnclear: "ask",
      sessionPanelOpen: true,
      notifyWhen: "needsYou",
    });
    const put = fakePut();
    const d = deps(put);
    const outcome = await importBrowserSettings(DEFAULT_SETTINGS, d);
    expect(put.mock.calls).toEqual([["agentsAtOnce", 9]]);
    expect(d.onSaved).toHaveBeenCalledTimes(1);
    expect(d.onSaved).toHaveBeenLastCalledWith({ ...DEFAULT_SETTINGS, agentsAtOnce: 9 });
    expect(outcome).toEqual({ sent: 1, failed: 0, message: null });
    expect(stored()).toBeNull();
  });

  it("clears th-settings when every stored value already matches", async () => {
    store({ ...DEFAULT_SETTINGS });
    const put = fakePut();
    const outcome = await importBrowserSettings(DEFAULT_SETTINGS, deps(put));
    expect(put).not.toHaveBeenCalled();
    expect(outcome).toEqual({ sent: 0, failed: 0, message: null });
    expect(stored()).toBeNull();
  });

  it("drops unknown keys and invalid values and sends the rest", async () => {
    store({ permissionMode: "yolo", agentsAtOnce: 99, theme: "light", notifyWhen: "everything" });
    const put = fakePut();
    await importBrowserSettings(DEFAULT_SETTINGS, deps(put));
    expect(put.mock.calls).toEqual([["notifyWhen", "everything"]]);
    expect(stored()).toBeNull();
  });

  it("drops th-settings that is not json", async () => {
    store("{not json");
    const put = fakePut();
    await importBrowserSettings(DEFAULT_SETTINGS, deps(put));
    expect(put).not.toHaveBeenCalled();
    expect(stored()).toBeNull();
  });

  it("keeps the values that failed and retries only them on the next load", async () => {
    store({ agentsAtOnce: 9, notifyWhen: "everything", theme: "light" });
    const failing = fakePut(["agentsAtOnce"]);
    const outcome = await importBrowserSettings(DEFAULT_SETTINGS, deps(failing));
    expect(failing.mock.calls).toEqual([
      ["agentsAtOnce", 9],
      ["notifyWhen", "everything"],
    ]);
    expect(outcome).toEqual({ sent: 2, failed: 1, message: "cannot reach the daemon: boom" });
    expect(JSON.parse(stored() ?? "null")).toEqual({ agentsAtOnce: 9 });

    const put = fakePut();
    const next = await importBrowserSettings({ ...DEFAULT_SETTINGS, notifyWhen: "everything" }, deps(put));
    expect(put.mock.calls).toEqual([["agentsAtOnce", 9]]);
    expect(next).toEqual({ sent: 1, failed: 0, message: null });
    expect(stored()).toBeNull();
  });

  it("warns and imports nothing when storage cannot be read", async () => {
    store({ agentsAtOnce: 9 });
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("storage denied");
    });
    const put = fakePut();
    const outcome = await importBrowserSettings(DEFAULT_SETTINGS, deps(put));
    expect(put).not.toHaveBeenCalled();
    expect(outcome).toEqual({ sent: 0, failed: 0, message: null });
    expect(console.warn).toHaveBeenCalledWith(expect.stringMatching(/^warn: /), expect.any(Error));
  });

  it("warns when th-settings cannot be cleared", async () => {
    store({ agentsAtOnce: 9 });
    vi.spyOn(Storage.prototype, "removeItem").mockImplementation(() => {
      throw new Error("storage denied");
    });
    const outcome = await importBrowserSettings(DEFAULT_SETTINGS, deps(fakePut()));
    expect(outcome).toEqual({ sent: 1, failed: 0, message: null });
    expect(console.warn).toHaveBeenCalledWith(expect.stringMatching(/^warn: /), expect.any(Error));
  });
});

describe("forgetStoredSetting", () => {
  it("drops one key from what is still waiting to be imported", () => {
    store({ agentsAtOnce: 9, notifyWhen: "everything" });
    forgetStoredSetting(localStorage, "agentsAtOnce");
    expect(JSON.parse(stored() ?? "null")).toEqual({ notifyWhen: "everything" });
  });

  it("clears th-settings when nothing is left", () => {
    store({ agentsAtOnce: 9, theme: "light" });
    forgetStoredSetting(localStorage, "agentsAtOnce");
    expect(stored()).toBeNull();
  });

  it("does nothing when nothing is stored or there is no storage", () => {
    forgetStoredSetting(localStorage, "agentsAtOnce");
    forgetStoredSetting(null, "agentsAtOnce");
    expect(stored()).toBeNull();
  });

  it("warns when storage fails", () => {
    store({ agentsAtOnce: 9, notifyWhen: "failed" });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("storage denied");
    });
    expect(() => forgetStoredSetting(localStorage, "agentsAtOnce")).not.toThrow();
    expect(console.warn).toHaveBeenCalledWith(expect.stringMatching(/^warn: /), expect.any(Error));
  });
});
