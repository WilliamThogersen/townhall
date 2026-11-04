import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import { ApiError } from "../api/errors";
import { useInfo } from "./useInfo";

const info = { home: "/Users/me/.townhall", repo: "/Users/me/Developer/townhall", repoKey: "townhall-1" };

afterEach(() => vi.restoreAllMocks());

describe("useInfo", () => {
  it("loads the daemon info", async () => {
    vi.spyOn(client, "getInfo").mockResolvedValue(info);
    const { result } = renderHook(() => useInfo());
    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.info).toEqual(info);
    expect(result.current.error).toBeNull();
  });

  it("reports an error and keeps the last info", async () => {
    const spy = vi.spyOn(client, "getInfo").mockResolvedValueOnce(info);
    const { result } = renderHook(() => useInfo());
    await waitFor(() => expect(result.current.info).toEqual(info));
    spy.mockRejectedValue(new ApiError("network", "daemon not reachable"));
    act(() => result.current.reload());
    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.error).toBe("daemon not reachable"));
    expect(result.current.loading).toBe(false);
    expect(result.current.info).toEqual(info);
  });

  it("clears the error once a reload works", async () => {
    const spy = vi.spyOn(client, "getInfo").mockRejectedValueOnce(new Error("down"));
    const { result } = renderHook(() => useInfo());
    await waitFor(() => expect(result.current.error).toBe("down"));
    spy.mockResolvedValue(info);
    act(() => result.current.reload());
    await waitFor(() => expect(result.current.info).toEqual(info));
    expect(result.current.error).toBeNull();
  });

  it("aborts the request when unmounted", async () => {
    let signal: AbortSignal | undefined;
    vi.spyOn(client, "getInfo").mockImplementation((s) => {
      signal = s;
      return new Promise(() => undefined);
    });
    const { unmount } = renderHook(() => useInfo());
    unmount();
    expect(signal?.aborted).toBe(true);
  });
});
