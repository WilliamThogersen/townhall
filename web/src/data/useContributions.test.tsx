import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aMarketplace, aPlugin, aSettingField, someContributions } from "../api/fixtures";
import * as marketsApi from "../api/marketplaces";
import * as pluginsApi from "../api/plugins";
import { TestDataProvider } from "./testing";
import { useContributions } from "./useContributions";
import { usePlugins } from "./usePlugins";

const wrapper = ({ children }: { children: ReactNode }) => <TestDataProvider>{children}</TestDataProvider>;

afterEach(() => vi.restoreAllMocks());

const slate = { plugin: "theme-slate@official", name: "theme-slate" };

describe("useContributions", () => {
  it("is empty and loading until the daemon answers, then holds what it sent", async () => {
    const got = someContributions({ themes: [slate] });
    vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(got);
    const { result } = renderHook(() => useContributions(), { wrapper });
    expect(result.current).toMatchObject({ loading: true, starters: [], themes: [], theme: null, settings: [] });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.themes).toEqual([slate]);
    expect(result.current.error).toBeNull();
  });

  it("keeps the message when the load fails", async () => {
    vi.spyOn(pluginsApi, "getContributions").mockRejectedValue(new Error("error: daemon down"));
    const { result } = renderHook(() => useContributions(), { wrapper });
    await waitFor(() => expect(result.current.error).toBe("daemon down"));
    expect(result.current.starters).toEqual([]);
  });

  it("reloads after a plugin is enabled, so its starters and theme appear", async () => {
    const load = vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(someContributions());
    vi.spyOn(pluginsApi, "listPlugins").mockResolvedValue({ available: [], installed: [aPlugin()] });
    vi.spyOn(marketsApi, "listMarketplaces").mockResolvedValue([aMarketplace()]);
    vi.spyOn(pluginsApi, "patchPlugin").mockResolvedValue(aPlugin({ enabled: true }));
    const { result } = renderHook(() => ({ contributions: useContributions(), plugins: usePlugins() }), { wrapper });
    await waitFor(() => expect(result.current.contributions.loading).toBe(false));
    await waitFor(() => expect(result.current.plugins.loading).toBe(false));
    load.mockResolvedValue(someContributions({ themes: [slate] }));
    await act(() => result.current.plugins.patch("lint@acme-tools", { enabled: true }));
    await waitFor(() => expect(result.current.contributions.themes).toEqual([slate]));
  });

  it("does not fetch contributions after a plugin change when nobody asked for them", async () => {
    const load = vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(someContributions());
    vi.spyOn(pluginsApi, "listPlugins").mockResolvedValue({ available: [], installed: [aPlugin()] });
    vi.spyOn(marketsApi, "listMarketplaces").mockResolvedValue([aMarketplace()]);
    vi.spyOn(pluginsApi, "patchPlugin").mockResolvedValue(aPlugin({ enabled: true }));
    const { result } = renderHook(() => usePlugins(), { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(() => result.current.patch("lint@acme-tools", { enabled: true }));
    expect(load).not.toHaveBeenCalled();
  });

  it("applies the answer of a theme choice at once, without another load", async () => {
    const load = vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(someContributions({ themes: [slate] }));
    const chosen = someContributions({ themes: [slate], theme: { plugin: slate.plugin, css: ":root {}\n" } });
    const put = vi.spyOn(pluginsApi, "putPluginTheme").mockResolvedValue(chosen);
    const { result } = renderHook(() => useContributions(), { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(() => result.current.chooseTheme(slate.plugin));
    expect(put).toHaveBeenCalledWith(slate.plugin);
    expect(result.current.theme).toEqual(chosen.theme);
    expect(load).toHaveBeenCalledTimes(1);
  });

  it("saves and resets a setting and shows the daemon's answer", async () => {
    const group = (value: number, set: boolean) => ({
      plugin: "lint@acme-tools",
      name: "lint",
      fields: [aSettingField({ value, set })],
    });
    vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(someContributions({ settings: [group(3, false)] }));
    const put = vi
      .spyOn(pluginsApi, "putPluginSetting")
      .mockResolvedValue(someContributions({ settings: [group(5, true)] }));
    const reset = vi
      .spyOn(pluginsApi, "resetPluginSetting")
      .mockResolvedValue(someContributions({ settings: [group(3, false)] }));
    const { result } = renderHook(() => useContributions(), { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    await act(() => result.current.setSetting("lint@acme-tools", "retries", 5));
    expect(put).toHaveBeenCalledWith("lint@acme-tools", "retries", 5);
    expect(result.current.settings[0]?.fields[0]).toMatchObject({ value: 5, set: true });
    await act(() => result.current.resetSetting("lint@acme-tools", "retries"));
    expect(reset).toHaveBeenCalledWith("lint@acme-tools", "retries");
    expect(result.current.settings[0]?.fields[0]).toMatchObject({ value: 3, set: false });
  });

  it("rejects a refused save with the api error and keeps what it had", async () => {
    vi.spyOn(pluginsApi, "getContributions").mockResolvedValue(someContributions({ themes: [slate] }));
    vi.spyOn(pluginsApi, "putPluginSetting").mockRejectedValue(new Error("retries: must be at most 9"));
    const { result } = renderHook(() => useContributions(), { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    await expect(result.current.setSetting("lint@acme-tools", "retries", 10)).rejects.toThrow("at most 9");
    expect(result.current.themes).toEqual([slate]);
  });
});
