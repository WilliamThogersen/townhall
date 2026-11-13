import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import type { PutSetting } from "./browserImport";
import { DEFAULT_SETTINGS, type Settings } from "./settings";
import { createSettingsStore, type SettingsStore } from "./settingsStore";
import { SettingsStoreContext, useLoadSettings, useSettings } from "./useSettings";

function fakeStore(initial: Partial<Settings> = {}) {
  let daemon: Settings = { ...DEFAULT_SETTINGS, ...initial };
  const load = vi.fn(async () => daemon);
  const put = vi.fn<PutSetting>(async (key, value) => {
    daemon = { ...daemon, [key]: value };
    return daemon;
  });
  const store = createSettingsStore({ load, put, storage: () => localStorage });
  return { store, load, put };
}

function wrapperFor(store: SettingsStore) {
  return function Wrapper({ children }: { readonly children: ReactNode }) {
    return <SettingsStoreContext value={store}>{children}</SettingsStoreContext>;
  };
}

function renderSettings(store: SettingsStore) {
  return renderHook(() => useSettings(), { wrapper: wrapperFor(store) });
}

beforeEach(() => localStorage.clear());

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("useSettings", () => {
  it("shows the defaults, then the daemon's settings", async () => {
    const { store } = fakeStore({ agentsAtOnce: 4 });
    const hook = renderSettings(store);
    expect(hook.result.current.settings).toEqual(DEFAULT_SETTINGS);
    await waitFor(() => expect(hook.result.current.settings.agentsAtOnce).toBe(4));
    expect(hook.result.current.loading).toBe(false);
    expect(hook.result.current.problem).toBeNull();
  });

  it("re-renders after update and saves it", async () => {
    const { store, put } = fakeStore();
    const hook = renderSettings(store);
    act(() => hook.result.current.update({ repoUnclear: "guess" }));
    expect(hook.result.current.settings.repoUnclear).toBe("guess");
    await waitFor(() => expect(put).toHaveBeenCalledWith("repoUnclear", "guess"));
  });

  it("re-renders when the store changes elsewhere", async () => {
    const { store } = fakeStore();
    const hook = renderSettings(store);
    act(() => store.update({ notifyWhen: "off" }));
    expect(hook.result.current.settings.notifyWhen).toBe("off");
    await waitFor(() => expect(hook.result.current.loading).toBe(false));
  });

  it("keeps its functions stable between renders", async () => {
    const { store } = fakeStore();
    const hook = renderSettings(store);
    const { update, retry, dismissProblem } = hook.result.current;
    act(() => hook.result.current.update({ agentsAtOnce: 10 }));
    await waitFor(() => expect(hook.result.current.loading).toBe(false));
    expect(hook.result.current.update).toBe(update);
    expect(hook.result.current.retry).toBe(retry);
    expect(hook.result.current.dismissProblem).toBe(dismissProblem);
  });

  it("loads once for every user and every render", async () => {
    const { store, load } = fakeStore();
    const wrapper = wrapperFor(store);
    const hooks = [1, 2, 3].map(() => renderHook(() => useSettings(), { wrapper }));
    hooks.forEach((hook) => hook.rerender());
    await waitFor(() => expect(hooks[0]?.result.current.loading).toBe(false));
    hooks.forEach((hook) => hook.rerender());
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("does not retry a failed load on its own", async () => {
    const { store, load } = fakeStore();
    load.mockRejectedValue(new ApiError("network", "cannot reach the daemon: boom"));
    const hook = renderSettings(store);
    await waitFor(() => expect(hook.result.current.problem?.kind).toBe("load"));
    hook.rerender();
    hook.rerender();
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(load).toHaveBeenCalledTimes(1);
    act(() => hook.result.current.retry());
    await waitFor(() => expect(load).toHaveBeenCalledTimes(2));
  });

  it("uses the app's daemon store without a provider", async () => {
    vi.resetModules();
    const answer = { ...DEFAULT_SETTINGS, agentsAtOnce: 12 };
    const fetch = vi.fn(async (_url: string, _init?: RequestInit) =>
      Response.json({ success: true, data: answer, error: null }),
    );
    vi.stubGlobal("fetch", fetch);
    const fresh = await import("./useSettings");
    const hook = renderHook(() => fresh.useSettings());
    await waitFor(() => expect(hook.result.current.settings.agentsAtOnce).toBe(12));
    expect(fetch).toHaveBeenCalledWith("/api/settings", expect.objectContaining({ method: "GET" }));
  });
});

describe("useLoadSettings", () => {
  it("loads the store once, without reading it", async () => {
    localStorage.setItem("th-settings", JSON.stringify({ agentsAtOnce: 9 }));
    const { store, load, put } = fakeStore();
    const hook = renderHook(() => useLoadSettings(), { wrapper: wrapperFor(store) });
    hook.rerender();
    await waitFor(() => expect(store.getState().settings.agentsAtOnce).toBe(9));
    expect(load).toHaveBeenCalledTimes(1);
    expect(put.mock.calls).toEqual([["agentsAtOnce", 9]]);
    expect(localStorage.getItem("th-settings")).toBeNull();
  });
});
