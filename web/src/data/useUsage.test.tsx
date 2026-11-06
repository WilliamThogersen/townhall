import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import { aUsage } from "../api/fixtures";
import * as usageApi from "../api/usage";
import { USAGE_POLL_MS, useUsage } from ".";
import { TestDataProvider } from "./testing";

const wrapper = ({ children }: { readonly children: ReactNode }) => (
  <TestDataProvider>{children}</TestDataProvider>
);

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useUsage", () => {
  it("loads the usage and asks again every minute", async () => {
    const get = vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage());
    const { result } = renderHook(() => useUsage(), { wrapper });
    expect(result.current).toEqual({ usage: null, loading: true, error: null });
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(result.current).toEqual({ usage: aUsage(), loading: false, error: null });
    expect(get).toHaveBeenCalledOnce();

    get.mockResolvedValue(aUsage({ session: { usedPercent: 31, resetsAt: null, ratePerHour: null } }));
    await act(() => vi.advanceTimersByTimeAsync(USAGE_POLL_MS));
    expect(get).toHaveBeenCalledTimes(2);
    expect(result.current.usage?.session?.usedPercent).toBe(31);
  });

  it("keeps the last reading when a later poll fails", async () => {
    const get = vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage());
    const { result } = renderHook(() => useUsage(), { wrapper });
    await act(() => vi.advanceTimersByTimeAsync(0));
    get.mockRejectedValue(new ApiError("network", "cannot reach the daemon: down"));
    await act(() => vi.advanceTimersByTimeAsync(USAGE_POLL_MS));
    expect(result.current).toEqual({ usage: aUsage(), loading: false, error: "cannot reach the daemon: down" });
  });

  it("stops polling when the screen goes away", async () => {
    const get = vi.spyOn(usageApi, "getUsage").mockResolvedValue(aUsage());
    const { unmount } = renderHook(() => useUsage(), { wrapper });
    await act(() => vi.advanceTimersByTimeAsync(0));
    unmount();
    await act(() => vi.advanceTimersByTimeAsync(USAGE_POLL_MS * 3));
    expect(get).toHaveBeenCalledOnce();
  });
});
