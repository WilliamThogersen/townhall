import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "../api/errors";
import * as searchApi from "../api/search";
import type { SearchHit } from "../api/search";
import { SEARCH_DELAY_MS, useMessageMatches } from "./useMessageMatches";

const hit = (conversationId: string): SearchHit => ({
  conversationId,
  title: conversationId,
  match: "message",
  entryId: 1,
  snippet: "text",
  updatedAt: "2026-11-04T10:42:05.000Z",
});

beforeEach(() => vi.useFakeTimers());

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

const settle = async (ms: number) => {
  await act(async () => {
    await vi.advanceTimersByTimeAsync(ms);
  });
};

describe("useMessageMatches", () => {
  it("asks the daemon 200 ms after the last change and keeps the newest answer", async () => {
    const search = vi.spyOn(searchApi, "searchConversations").mockResolvedValue([hit("a"), hit("b")]);
    const { result, rerender } = renderHook(({ q }) => useMessageMatches(q), { initialProps: { q: "che" } });
    await settle(SEARCH_DELAY_MS - 50);
    rerender({ q: "check " });
    await settle(SEARCH_DELAY_MS - 1);
    expect(search).not.toHaveBeenCalled();
    await settle(1);
    expect(search).toHaveBeenCalledOnce();
    expect(search.mock.calls[0]?.[0]).toBe("check");
    expect([...result.current.ids]).toEqual(["a", "b"]);
    expect(result.current.error).toBeNull();
  });

  it("aborts the request of an older text and drops its answer", async () => {
    const signals: AbortSignal[] = [];
    vi.spyOn(searchApi, "searchConversations").mockImplementation((_q, signal) => {
      if (signal) signals.push(signal);
      return new Promise<readonly SearchHit[]>(() => undefined);
    });
    const { result, rerender } = renderHook(({ q }) => useMessageMatches(q), { initialProps: { q: "one" } });
    await settle(SEARCH_DELAY_MS);
    rerender({ q: "two" });
    expect(signals[0]?.aborted).toBe(true);
    expect(result.current.ids.size).toBe(0);
  });

  it("asks nothing for a blank text", async () => {
    const search = vi.spyOn(searchApi, "searchConversations").mockResolvedValue([hit("a")]);
    const { result } = renderHook(() => useMessageMatches("   "));
    await settle(SEARCH_DELAY_MS * 2);
    expect(search).not.toHaveBeenCalled();
    expect(result.current.ids.size).toBe(0);
  });

  it("reports a failed search and clears it on the next answer", async () => {
    const search = vi
      .spyOn(searchApi, "searchConversations")
      .mockRejectedValueOnce(new ApiError("network", "cannot reach the daemon: down"))
      .mockResolvedValue([hit("c")]);
    const { result, rerender } = renderHook(({ q }) => useMessageMatches(q), { initialProps: { q: "x" } });
    await settle(SEARCH_DELAY_MS);
    expect(result.current.error).toBe("cannot reach the daemon: down");
    rerender({ q: "xy" });
    await settle(SEARCH_DELAY_MS);
    expect(search).toHaveBeenCalledTimes(2);
    expect(result.current.error).toBeNull();
    expect([...result.current.ids]).toEqual(["c"]);
  });
});
