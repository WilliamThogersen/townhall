import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as artifactsApi from "../api/artifacts";
import { anArtifact } from "../api/fixtures";
import { ARTIFACTS_POLL_MS, useArtifacts } from ".";

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useArtifacts", () => {
  it("loads the artifacts of the task and polls while it is shown", async () => {
    const list = vi.spyOn(artifactsApi, "listArtifacts").mockResolvedValue([anArtifact()]);
    const { result } = renderHook(() => useArtifacts("fix-login"));
    await act(() => vi.advanceTimersByTimeAsync(ARTIFACTS_POLL_MS));
    expect(result.current).toEqual({ artifacts: [anArtifact()], loading: false, error: null });
    expect(list).toHaveBeenCalledTimes(2);
    expect(list.mock.calls[0]?.[0]).toBe("fix-login");
  });

  it("does nothing without a task and stops polling when it goes away", async () => {
    const list = vi.spyOn(artifactsApi, "listArtifacts").mockResolvedValue([]);
    const { result, rerender } = renderHook(({ task }) => useArtifacts(task), {
      initialProps: { task: "fix-login" as string | null },
    });
    await act(() => vi.advanceTimersByTimeAsync(0));
    rerender({ task: null });
    await act(() => vi.advanceTimersByTimeAsync(ARTIFACTS_POLL_MS * 3));
    expect(list).toHaveBeenCalledTimes(1);
    expect(result.current).toEqual({ artifacts: [], loading: false, error: null });
  });

  it("reports why it could not load", async () => {
    vi.spyOn(artifactsApi, "listArtifacts").mockRejectedValue(new Error("daemon down"));
    const { result } = renderHook(() => useArtifacts("fix-login"));
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(result.current.error).toContain("daemon down");
    expect(result.current.artifacts).toEqual([]);
  });
});
