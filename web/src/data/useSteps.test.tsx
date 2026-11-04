import { act, renderHook } from "@testing-library/react";
import type { ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { aStep } from "../api/fixtures";
import * as stepsApi from "../api/steps";
import { STEPS_POLL_MS, useSteps } from ".";
import { TestDataProvider } from "./testing";

const wrapper = ({ children }: { readonly children: ReactNode }) => (
  <TestDataProvider>{children}</TestDataProvider>
);

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("useSteps", () => {
  it("loads once when not live", async () => {
    const get = vi.spyOn(stepsApi, "getSteps").mockResolvedValue([aStep()]);
    const { result } = renderHook(() => useSteps("fix-login", { repo: "shop" }), { wrapper });
    await act(() => vi.advanceTimersByTimeAsync(STEPS_POLL_MS * 3));
    expect(result.current).toEqual({ steps: [aStep()], loading: false, error: null });
    expect(get).toHaveBeenCalledOnce();
    expect(get.mock.calls[0]?.slice(0, 2)).toEqual(["fix-login", "shop"]);
  });

  it("polls while live and stops when the agent goes away", async () => {
    const get = vi.spyOn(stepsApi, "getSteps").mockResolvedValue([aStep()]);
    const { result, rerender } = renderHook(({ agent }) => useSteps(agent, { live: true }), {
      wrapper,
      initialProps: { agent: "fix-login" as string | null },
    });
    await act(() => vi.advanceTimersByTimeAsync(STEPS_POLL_MS));
    await act(() => vi.advanceTimersByTimeAsync(STEPS_POLL_MS));
    expect(get).toHaveBeenCalledTimes(3);
    rerender({ agent: null });
    await act(() => vi.advanceTimersByTimeAsync(STEPS_POLL_MS * 2));
    expect(get).toHaveBeenCalledTimes(3);
    expect(result.current).toEqual({ steps: [], loading: false, error: null });
  });
});
