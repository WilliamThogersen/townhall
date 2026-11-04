import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { stubColorScheme } from "./testMatchMedia";

type ThemeModule = typeof import("./theme");

async function loadTheme(): Promise<ThemeModule> {
  vi.resetModules();
  return import("./theme");
}

const currentTheme = () => document.documentElement.getAttribute("data-theme");

function failStorage(method: "getItem" | "setItem") {
  vi.spyOn(Storage.prototype, method).mockImplementation(() => {
    throw new Error("storage denied");
  });
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
  vi.spyOn(console, "warn").mockImplementation(() => undefined);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("readThemeChoice", () => {
  it("defaults to dark when nothing is stored", async () => {
    const { readThemeChoice } = await loadTheme();
    expect(readThemeChoice()).toBe("dark");
  });

  it("returns a stored choice", async () => {
    const { readThemeChoice, THEME_STORAGE_KEY } = await loadTheme();
    localStorage.setItem(THEME_STORAGE_KEY, "light");
    expect(readThemeChoice()).toBe("light");
    localStorage.setItem(THEME_STORAGE_KEY, "system");
    expect(readThemeChoice()).toBe("system");
  });

  it("uses th-theme as the storage key", async () => {
    const { THEME_STORAGE_KEY } = await loadTheme();
    expect(THEME_STORAGE_KEY).toBe("th-theme");
  });

  it("falls back to dark for an invalid stored value", async () => {
    const { readThemeChoice } = await loadTheme();
    localStorage.setItem("th-theme", "blue");
    expect(readThemeChoice()).toBe("dark");
  });

  it("falls back to dark when storage throws on read", async () => {
    const { readThemeChoice } = await loadTheme();
    failStorage("getItem");
    expect(readThemeChoice()).toBe("dark");
    expect(console.warn).toHaveBeenCalled();
  });
});

describe("resolveTheme", () => {
  it("keeps an explicit choice", async () => {
    const { resolveTheme } = await loadTheme();
    stubColorScheme(true);
    expect(resolveTheme("dark")).toBe("dark");
    stubColorScheme(false);
    expect(resolveTheme("light")).toBe("light");
  });

  it("follows the os when the choice is system", async () => {
    const { resolveTheme } = await loadTheme();
    stubColorScheme(true);
    expect(resolveTheme("system")).toBe("light");
    stubColorScheme(false);
    expect(resolveTheme("system")).toBe("dark");
  });

  it("resolves system to dark without matchMedia", async () => {
    const { resolveTheme } = await loadTheme();
    vi.stubGlobal("matchMedia", undefined);
    expect(resolveTheme("system")).toBe("dark");
  });
});

describe("applyTheme", () => {
  it("sets data-theme on the root element", async () => {
    const { applyTheme } = await loadTheme();
    applyTheme("light");
    expect(currentTheme()).toBe("light");
    applyTheme("dark");
    expect(currentTheme()).toBe("dark");
  });
});

describe("saveThemeChoice", () => {
  it("stores, applies and notifies subscribers", async () => {
    const { saveThemeChoice, subscribeTheme, getThemeState } = await loadTheme();
    const listener = vi.fn();
    subscribeTheme(listener);
    saveThemeChoice("light");
    expect(localStorage.getItem("th-theme")).toBe("light");
    expect(currentTheme()).toBe("light");
    expect(listener).toHaveBeenCalledTimes(1);
    expect(getThemeState()).toEqual({ choice: "light", resolved: "light" });
  });

  it("stops notifying after unsubscribe", async () => {
    const { saveThemeChoice, subscribeTheme } = await loadTheme();
    const kept = vi.fn();
    const dropped = vi.fn();
    subscribeTheme(kept);
    const unsubscribe = subscribeTheme(dropped);
    unsubscribe();
    saveThemeChoice("light");
    expect(kept).toHaveBeenCalledTimes(1);
    expect(dropped).not.toHaveBeenCalled();
  });

  it("still applies and notifies when storage throws on write", async () => {
    const { saveThemeChoice, subscribeTheme, getThemeState } = await loadTheme();
    failStorage("setItem");
    const listener = vi.fn();
    subscribeTheme(listener);
    saveThemeChoice("light");
    expect(currentTheme()).toBe("light");
    expect(listener).toHaveBeenCalledTimes(1);
    expect(getThemeState().choice).toBe("light");
    expect(console.warn).toHaveBeenCalled();
  });

  it("resolves system against the os", async () => {
    const { saveThemeChoice, getThemeState } = await loadTheme();
    stubColorScheme(true);
    saveThemeChoice("system");
    expect(currentTheme()).toBe("light");
    expect(getThemeState()).toEqual({ choice: "system", resolved: "light" });
  });
});

describe("getThemeState", () => {
  it("starts from the stored choice and stays the same object until a change", async () => {
    const { getThemeState } = await loadTheme();
    localStorage.setItem("th-theme", "light");
    const first = getThemeState();
    expect(first).toEqual({ choice: "light", resolved: "light" });
    expect(getThemeState()).toBe(first);
  });
});

describe("initTheme", () => {
  it("applies dark by default", async () => {
    const { initTheme } = await loadTheme();
    initTheme();
    expect(currentTheme()).toBe("dark");
  });

  it("applies the stored choice", async () => {
    const { initTheme } = await loadTheme();
    localStorage.setItem("th-theme", "light");
    initTheme();
    expect(currentTheme()).toBe("light");
  });

  it("follows os changes while the choice is system", async () => {
    const { initTheme, subscribeTheme, getThemeState } = await loadTheme();
    const os = stubColorScheme(false);
    localStorage.setItem("th-theme", "system");
    initTheme();
    expect(currentTheme()).toBe("dark");
    const listener = vi.fn();
    subscribeTheme(listener);
    os.setLight(true);
    expect(currentTheme()).toBe("light");
    expect(getThemeState()).toEqual({ choice: "system", resolved: "light" });
    expect(listener).toHaveBeenCalledTimes(1);
  });

  it("ignores os changes once the choice is not system", async () => {
    const { initTheme, saveThemeChoice, subscribeTheme } = await loadTheme();
    const os = stubColorScheme(false);
    localStorage.setItem("th-theme", "system");
    initTheme();
    saveThemeChoice("dark");
    const listener = vi.fn();
    subscribeTheme(listener);
    os.setLight(true);
    expect(currentTheme()).toBe("dark");
    expect(listener).not.toHaveBeenCalled();
  });

  it("watches the os only once when called twice", async () => {
    const { initTheme } = await loadTheme();
    const os = stubColorScheme(false);
    initTheme();
    initTheme();
    expect(os.listenerCount()).toBe(1);
  });

  it("works without matchMedia", async () => {
    const { initTheme } = await loadTheme();
    vi.stubGlobal("matchMedia", undefined);
    localStorage.setItem("th-theme", "system");
    initTheme();
    expect(currentTheme()).toBe("dark");
  });
});
