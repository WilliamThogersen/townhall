import { act, render, renderHook, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { aTask } from "../../api/fixtures";
import * as stepsApi from "../../api/steps";
import { TestDataProvider } from "../../data/testing";
import { ControlHead } from "./ControlHead";
import { LoopStrip } from "./LoopStrip";
import { repoChoices } from "./RepoFilter";
import { loopSummary } from "../../tasks/stage";
import { LIVE_STEPS_MS, StepsLoader } from "./StepsLoader";
import { controlHref } from "./useControlFilter";
import { controlHints } from "./useControlKeys";
import { useDocumentVisible } from "./useDocumentVisible";
import { NOW_TICK_MS, useNow } from "./useNow";

afterEach(() => {
  vi.useRealTimers();
  vi.restoreAllMocks();
});

describe("ControlHead", () => {
  it.each([
    ["open", "● live", "open"],
    ["connecting", "connecting…", null],
    ["closed", "reconnecting…", null],
  ] as const)("says %s as %s", (connection, words, status) => {
    const { container } = render(<ControlHead caption={null} connection={connection} allConversationsHref={null} />);
    const stream = screen.getByText(words);
    expect(stream).toHaveClass("th-stream");
    expect(stream.getAttribute("data-status")).toBe(status);
    expect(container.querySelector(".th-control-cap")).toBeNull();
    expect(container.firstChild).toHaveAttribute("data-tauri-drag-region", "deep");
  });
});

describe("LoopStrip", () => {
  it("leaves needs you out when nothing waits on you and dims empty stages", () => {
    const { container } = render(<LoopStrip loop={loopSummary([aTask({ state: "working" })], () => null)} />);
    expect(container.querySelector(".th-loop-stage--you")).toBeNull();
    const stages = [...container.querySelectorAll(".th-loop-stage")];
    expect(stages.map((s) => s.getAttribute("data-active"))).toEqual([null, "true", null, null, null]);
    expect(container.querySelectorAll(".th-loop-arrow")).toHaveLength(4);
  });
});

describe("small helpers", () => {
  it("builds control center links from the filter", () => {
    expect(controlHref({ conversationId: null, repo: null })).toBe("/townhall");
    expect(controlHref({ conversationId: "c 1", repo: "shop" })).toBe("/townhall?c=c+1&repo=shop");
  });

  it("keeps an unknown repo from the url in the menu", () => {
    expect(repoChoices(["shop"], "Shop")).toEqual(["shop"]);
    expect(repoChoices(["shop"], "gone")).toEqual(["shop", "gone"]);
    expect(repoChoices(["shop"], null)).toEqual(["shop"]);
  });

  it("only hints the keys that work", () => {
    expect(controlHints(false).map((h) => h.keys)).toEqual(["f"]);
    expect(controlHints(true).map((h) => h.label)).toEqual(["rows", "fold", "open conversation", "filter repo"]);
  });
});

describe("useNow", () => {
  it("ticks", () => {
    vi.useFakeTimers();
    vi.setSystemTime(1_000_000);
    const { result } = renderHook(() => useNow());
    expect(result.current).toBe(1_000_000);
    act(() => vi.advanceTimersByTime(NOW_TICK_MS));
    expect(result.current).toBe(1_000_000 + NOW_TICK_MS);
  });
});

describe("useDocumentVisible", () => {
  it("follows the document's visibility", () => {
    let state: DocumentVisibilityState = "visible";
    vi.spyOn(document, "visibilityState", "get").mockImplementation(() => state);
    const { result } = renderHook(() => useDocumentVisible());
    expect(result.current).toBe(true);
    act(() => {
      state = "hidden";
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(result.current).toBe(false);
  });
});

describe("StepsLoader", () => {
  it("polls every 5 s while live, stops when not, and reports each answer", async () => {
    vi.useFakeTimers();
    const getSteps = vi.spyOn(stepsApi, "getSteps").mockResolvedValue([]);
    const onSteps = vi.fn();
    const task = aTask({ name: "Fix-Login", repo: "shop", state: "working" });
    const view = (live: boolean) => (
      <TestDataProvider>
        <StepsLoader task={task} live={live} onSteps={onSteps} />
      </TestDataProvider>
    );
    const { rerender } = render(view(true));
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(getSteps).toHaveBeenCalledTimes(1);
    expect(getSteps).toHaveBeenLastCalledWith("Fix-Login", "shop", expect.any(AbortSignal));
    await act(() => vi.advanceTimersByTimeAsync(LIVE_STEPS_MS));
    expect(getSteps).toHaveBeenCalledTimes(2);
    rerender(view(false));
    await act(() => vi.advanceTimersByTimeAsync(LIVE_STEPS_MS * 3));
    expect(getSteps).toHaveBeenCalledTimes(2);
    expect(onSteps).toHaveBeenLastCalledWith("fix-login", { steps: [], loading: false, error: null });
  });
});
