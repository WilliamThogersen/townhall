import { act, renderHook, waitFor } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aMarketplace, anAvailablePlugin, aPlugin, aPreview } from "../api/fixtures";
import * as marketsApi from "../api/marketplaces";
import * as pluginsApi from "../api/plugins";
import { TestDataProvider } from "./testing";
import { usePlugins } from "./usePlugins";

const wrapper = ({ children }: { children: ReactNode }) => <TestDataProvider>{children}</TestDataProvider>;

afterEach(() => vi.restoreAllMocks());

function stubDaemon() {
  const list = vi.spyOn(pluginsApi, "listPlugins").mockResolvedValue({
    available: [anAvailablePlugin()],
    installed: [],
  });
  vi.spyOn(marketsApi, "listMarketplaces").mockResolvedValue([aMarketplace()]);
  return list;
}

describe("usePlugins", () => {
  it("is empty and loading until the daemon answers, then lists everything", async () => {
    stubDaemon();
    const { result } = renderHook(() => usePlugins(), { wrapper });
    expect(result.current).toMatchObject({ loading: true, marketplaces: [], available: [], installed: [] });
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.marketplaces).toEqual([aMarketplace()]);
    expect(result.current.available).toEqual([anAvailablePlugin()]);
    expect(result.current.error).toBeNull();
  });

  it("keeps the message when a load fails", async () => {
    vi.spyOn(pluginsApi, "listPlugins").mockRejectedValue(new Error("error: daemon down"));
    vi.spyOn(marketsApi, "listMarketplaces").mockResolvedValue([]);
    const { result } = renderHook(() => usePlugins(), { wrapper });
    await waitFor(() => expect(result.current.error).toBe("daemon down"));
  });

  it("reloads after installing so the plugin shows as installed", async () => {
    const list = stubDaemon();
    vi.spyOn(pluginsApi, "installPlugin").mockResolvedValue(aPlugin());
    const { result } = renderHook(() => usePlugins(), { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    list.mockResolvedValue({ available: [anAvailablePlugin({ installed: true })], installed: [aPlugin()] });
    await act(() => result.current.install("lint@acme-tools", "sha256:aaa"));
    expect(pluginsApi.installPlugin).toHaveBeenCalledWith("lint@acme-tools", "sha256:aaa");
    await waitFor(() => expect(result.current.installed).toEqual([aPlugin()]));
  });

  it("passes the approved hash through an update and previews without reloading", async () => {
    const list = stubDaemon();
    vi.spyOn(pluginsApi, "previewPlugin").mockResolvedValue(aPreview());
    vi.spyOn(pluginsApi, "updatePlugin").mockResolvedValue({
      outcome: "updated",
      sha: "2",
      added: [],
      permissionsHash: null,
      plugin: aPlugin(),
    });
    const { result } = renderHook(() => usePlugins(), { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    const calls = list.mock.calls.length;
    expect(await result.current.preview("lint@acme-tools")).toEqual(aPreview());
    expect(list.mock.calls.length).toBe(calls);
    await act(() => result.current.update("lint@acme-tools", "sha256:bbb"));
    expect(pluginsApi.updatePlugin).toHaveBeenCalledWith("lint@acme-tools", "sha256:bbb");
  });

  it("rejects an action with the api error and keeps the list", async () => {
    stubDaemon();
    vi.spyOn(marketsApi, "removeMarketplace").mockRejectedValue(new Error("it still has installed plugins"));
    const { result } = renderHook(() => usePlugins(), { wrapper });
    await waitFor(() => expect(result.current.loading).toBe(false));
    await expect(result.current.removeMarketplace("acme-tools")).rejects.toThrow("still has installed plugins");
    expect(result.current.marketplaces).toEqual([aMarketplace()]);
  });
});
