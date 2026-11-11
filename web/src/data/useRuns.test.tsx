import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { aRun } from "../api/fixtures";
import * as runsApi from "../api/runs";
import { RUNS_POLL_MS, useRuns } from "./useRuns";

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useRuns", () => {
  it("loads the runs of the conversation and polls while it is shown", async () => {
    const list = vi.spyOn(runsApi, "listRuns").mockResolvedValue([aRun()]);
    const { result } = renderHook(() => useRuns("c1", ""));
    await act(() => vi.advanceTimersByTimeAsync(RUNS_POLL_MS));
    expect(result.current).toEqual({ runs: [aRun()], loading: false, error: null });
    expect(list).toHaveBeenCalledTimes(2);
    expect(list.mock.calls[0]?.[0]).toBe("c1");
  });

  it("loads again when what it watches changes, keeping the runs it has meanwhile", async () => {
    const list = vi.spyOn(runsApi, "listRuns").mockResolvedValue([aRun({ state: "running" })]);
    const { result, rerender } = renderHook(({ watch }) => useRuns("c1", watch), { initialProps: { watch: "a" } });
    await act(() => vi.advanceTimersByTimeAsync(0));
    list.mockResolvedValue([aRun({ state: "finished" })]);
    rerender({ watch: "b" });
    expect(result.current.runs[0]?.state).toBe("running");
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(result.current.runs[0]?.state).toBe("finished");
    expect(list).toHaveBeenCalledTimes(2);
  });

  it("reports why it could not load and shows no runs", async () => {
    vi.spyOn(runsApi, "listRuns").mockRejectedValue(new Error("daemon down"));
    const { result } = renderHook(() => useRuns("c1", ""));
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(result.current.error).toContain("daemon down");
    expect(result.current.runs).toEqual([]);
  });
});
