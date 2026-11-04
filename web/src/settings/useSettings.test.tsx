import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";

async function renderSettings() {
  vi.resetModules();
  const { useSettings } = await import("./useSettings");
  const store = await import("./settingsStore");
  const hook = renderHook(() => useSettings());
  return { hook, store };
}

beforeEach(() => {
  localStorage.clear();
});

describe("useSettings", () => {
  it("returns the current settings", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ agentsAtOnce: 4 }));
    const { hook } = await renderSettings();
    expect(hook.result.current.settings.agentsAtOnce).toBe(4);
  });

  it("re-renders after update", async () => {
    const { hook } = await renderSettings();
    act(() => hook.result.current.update({ repoUnclear: "guess" }));
    expect(hook.result.current.settings.repoUnclear).toBe("guess");
  });

  it("re-renders when the store changes elsewhere", async () => {
    const { hook, store } = await renderSettings();
    act(() => store.updateSettings({ notifyWhen: "everything" }));
    expect(hook.result.current.settings.notifyWhen).toBe("everything");
  });

  it("keeps update stable between renders", async () => {
    const { hook } = await renderSettings();
    const first = hook.result.current.update;
    act(() => hook.result.current.update({ agentsAtOnce: 10 }));
    expect(hook.result.current.update).toBe(first);
  });
});
