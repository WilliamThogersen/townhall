import { act, renderHook, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import * as client from "../api/client";
import { ApiError } from "../api/errors";
import { useAgents } from "./useAgents";

const a = {
  name: "a", kind: null, branch: "b", worktree: "/w", alive: true, repo: "demo", state: { kind: "working" },
} as const;

afterEach(() => vi.restoreAllMocks());

describe("useAgents", () => {
  it("loads agents then clears loading", async () => {
    vi.spyOn(client, "listAgents").mockResolvedValue([a]);
    const { result } = renderHook(() => useAgents(60_000));
    expect(result.current.loading).toBe(true);
    await waitFor(() => expect(result.current.loading).toBe(false));
    expect(result.current.agents).toEqual([a]);
    expect(result.current.error).toBeNull();
  });

  it("surfaces errors and keeps the last good list", async () => {
    const spy = vi.spyOn(client, "listAgents").mockResolvedValueOnce([a]);
    const { result } = renderHook(() => useAgents(60_000));
    await waitFor(() => expect(result.current.agents).toHaveLength(1));
    spy.mockRejectedValue(new ApiError("network", "down"));
    await act(() => result.current.refresh());
    expect(result.current.error).toBe("down");
    expect(result.current.agents).toEqual([a]);
  });

  it("polls on the interval", async () => {
    vi.useFakeTimers();
    const spy = vi.spyOn(client, "listAgents").mockResolvedValue([]);
    renderHook(() => useAgents(3000));
    await act(() => vi.advanceTimersByTimeAsync(6100));
    expect(spy.mock.calls.length).toBeGreaterThanOrEqual(3);
    vi.useRealTimers();
  });
});
