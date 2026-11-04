import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

type StoreModule = typeof import("./settingsStore");

async function loadStore(): Promise<StoreModule> {
  vi.resetModules();
  return import("./settingsStore");
}

function failStorage(method: "getItem" | "setItem") {
  vi.spyOn(Storage.prototype, method).mockImplementation(() => {
    throw new Error("storage denied");
  });
}

const stored = () => JSON.parse(localStorage.getItem("th-settings") ?? "null") as unknown;

beforeEach(() => {
  localStorage.clear();
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("settings store", () => {
  it("uses th-settings as its storage key", async () => {
    const { SETTINGS_STORAGE_KEY } = await loadStore();
    expect(SETTINGS_STORAGE_KEY).toBe("th-settings");
  });

  it("starts from the defaults when nothing is stored", async () => {
    const { getSettings } = await loadStore();
    const { DEFAULT_SETTINGS } = await import("./settings");
    expect(getSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("reads what is stored", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ agentsAtOnce: 9, repoUnclear: "guess" }));
    const { getSettings } = await loadStore();
    expect(getSettings().agentsAtOnce).toBe(9);
    expect(getSettings().repoUnclear).toBe("guess");
    expect(getSettings().permissionMode).toBe("acceptEdits");
  });

  it("falls back to the defaults on invalid json", async () => {
    localStorage.setItem("th-settings", "{not json");
    const { getSettings } = await loadStore();
    const { DEFAULT_SETTINGS } = await import("./settings");
    expect(getSettings()).toEqual(DEFAULT_SETTINGS);
  });

  it("falls back per field on invalid values", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ agentsAtOnce: 99, notifyWhen: "everything" }));
    const { getSettings } = await loadStore();
    expect(getSettings().agentsAtOnce).toBe(6);
    expect(getSettings().notifyWhen).toBe("everything");
  });

  it("returns the same object until something changes", async () => {
    const { getSettings, updateSettings } = await loadStore();
    const first = getSettings();
    expect(getSettings()).toBe(first);
    updateSettings({ agentsAtOnce: 7 });
    expect(getSettings()).not.toBe(first);
    expect(first.agentsAtOnce).toBe(6);
  });

  it("saves every change as validated json", async () => {
    const { getSettings, updateSettings } = await loadStore();
    updateSettings({ permissionMode: "bypassPermissions", workWithoutAsking: true });
    expect(getSettings().permissionMode).toBe("bypassPermissions");
    expect(stored()).toEqual({ ...getSettings() });
  });

  it("keeps the saved values in a new session", async () => {
    const first = await loadStore();
    first.updateSettings({ sessionPanelOpen: false, notifyWhen: "failed" });
    const second = await loadStore();
    expect(second.getSettings().sessionPanelOpen).toBe(false);
    expect(second.getSettings().notifyWhen).toBe("failed");
  });

  it("keeps agents at once inside its limits", async () => {
    const { getSettings, updateSettings } = await loadStore();
    updateSettings({ agentsAtOnce: 80 });
    expect(getSettings().agentsAtOnce).toBe(50);
    updateSettings({ agentsAtOnce: 0 });
    expect(getSettings().agentsAtOnce).toBe(1);
  });

  it("works in memory when storage cannot be read", async () => {
    failStorage("getItem");
    const { getSettings, updateSettings } = await loadStore();
    expect(getSettings().agentsAtOnce).toBe(6);
    updateSettings({ agentsAtOnce: 3 });
    expect(getSettings().agentsAtOnce).toBe(3);
    expect(console.warn).toHaveBeenCalled();
  });

  it("works in memory when storage cannot be written", async () => {
    failStorage("setItem");
    const { getSettings, updateSettings } = await loadStore();
    updateSettings({ repoUnclear: "guess" });
    expect(getSettings().repoUnclear).toBe("guess");
    expect(console.warn).toHaveBeenCalled();
  });

  it("tells subscribers about each change until they leave", async () => {
    const { subscribeSettings, updateSettings } = await loadStore();
    const first = vi.fn();
    const second = vi.fn();
    const leave = subscribeSettings(first);
    subscribeSettings(second);
    updateSettings({ agentsAtOnce: 8 });
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
    leave();
    updateSettings({ agentsAtOnce: 9 });
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(2);
  });

  it("skips saving and subscribers when nothing changes", async () => {
    const { subscribeSettings, updateSettings } = await loadStore();
    const listener = vi.fn();
    subscribeSettings(listener);
    updateSettings({ agentsAtOnce: 6 });
    expect(listener).not.toHaveBeenCalled();
    expect(localStorage.getItem("th-settings")).toBeNull();
  });
});
