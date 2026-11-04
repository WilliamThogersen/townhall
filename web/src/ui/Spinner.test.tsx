import { act, cleanup, render, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SPINNER_FRAMES, SPINNER_STATIC, Spinner, useSpinnerFrame } from "./Spinner";

const FRAME_MS = 80;

type ChangeListener = (event: { matches: boolean }) => void;

function stubReducedMotion(initial: boolean) {
  const listeners = new Set<ChangeListener>();
  const query = {
    matches: initial,
    media: "(prefers-reduced-motion: reduce)",
    addEventListener: (_type: string, listener: ChangeListener) => listeners.add(listener),
    removeEventListener: (_type: string, listener: ChangeListener) => listeners.delete(listener),
  };
  vi.stubGlobal("matchMedia", vi.fn(() => query));
  return {
    change(next: boolean) {
      query.matches = next;
      listeners.forEach((listener) => listener({ matches: next }));
    },
    listenerCount: () => listeners.size,
  };
}

function frameTimerCalls(spy: { mock: { calls: unknown[][] } }) {
  return spy.mock.calls.filter((call) => call[1] === FRAME_MS);
}

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("SPINNER_FRAMES", () => {
  it("cycles the braille frames and freezes on the full cell", () => {
    expect(SPINNER_FRAMES).toEqual(["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]);
    expect(SPINNER_STATIC).toBe("⠿");
  });
});

describe("Spinner", () => {
  it("renders the first frame in an aria-hidden th-spin", () => {
    const { container } = render(<Spinner />);
    const spin = container.querySelector(".th-spin");
    expect(spin).not.toBeNull();
    expect(spin).toHaveAttribute("aria-hidden", "true");
    expect(spin).toHaveTextContent(SPINNER_FRAMES[0]!);
  });

  it("advances one frame every 80ms and wraps around", () => {
    const { container } = render(<Spinner />);
    const spin = () => container.querySelector(".th-spin")?.textContent;
    act(() => vi.advanceTimersByTime(FRAME_MS - 1));
    expect(spin()).toBe(SPINNER_FRAMES[0]);
    act(() => vi.advanceTimersByTime(1));
    expect(spin()).toBe(SPINNER_FRAMES[1]);
    act(() => vi.advanceTimersByTime(FRAME_MS * (SPINNER_FRAMES.length - 1)));
    expect(spin()).toBe(SPINNER_FRAMES[0]);
  });

  it("shares one interval between every mounted spinner", () => {
    const setSpy = vi.spyOn(globalThis, "setInterval");
    const { container } = render(
      <>
        <Spinner />
        <Spinner />
      </>,
    );
    expect(frameTimerCalls(setSpy)).toHaveLength(1);
    act(() => vi.advanceTimersByTime(FRAME_MS * 3));
    const frames = Array.from(container.querySelectorAll(".th-spin"), (el) => el.textContent);
    expect(frames).toEqual([SPINNER_FRAMES[3], SPINNER_FRAMES[3]]);
  });

  it("keeps the interval until the last spinner unmounts", () => {
    const setSpy = vi.spyOn(globalThis, "setInterval");
    const clearSpy = vi.spyOn(globalThis, "clearInterval");
    const first = render(<Spinner />);
    const second = render(<Spinner />);
    expect(frameTimerCalls(setSpy)).toHaveLength(1);
    first.unmount();
    expect(clearSpy).not.toHaveBeenCalled();
    expect(vi.getTimerCount()).toBe(1);
    second.unmount();
    expect(clearSpy).toHaveBeenCalledTimes(1);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("starts again from the first frame after every spinner unmounted", () => {
    const first = render(<Spinner />);
    act(() => vi.advanceTimersByTime(FRAME_MS * 2));
    first.unmount();
    const { container } = render(<Spinner />);
    expect(container.querySelector(".th-spin")).toHaveTextContent(SPINNER_FRAMES[0]!);
  });
});

describe("useSpinnerFrame with reduced motion", () => {
  it("returns the static frame and starts no timer", () => {
    stubReducedMotion(true);
    const setSpy = vi.spyOn(globalThis, "setInterval");
    const { result } = renderHook(() => useSpinnerFrame());
    expect(result.current).toBe(SPINNER_STATIC);
    act(() => vi.advanceTimersByTime(FRAME_MS * 5));
    expect(result.current).toBe(SPINNER_STATIC);
    expect(frameTimerCalls(setSpy)).toHaveLength(0);
    expect(vi.getTimerCount()).toBe(0);
  });

  it("animates when the media query does not match", () => {
    stubReducedMotion(false);
    const { result } = renderHook(() => useSpinnerFrame());
    expect(result.current).toBe(SPINNER_FRAMES[0]);
    act(() => vi.advanceTimersByTime(FRAME_MS));
    expect(result.current).toBe(SPINNER_FRAMES[1]);
  });

  it("follows a change of the preference while mounted", () => {
    const motion = stubReducedMotion(false);
    const { result, unmount } = renderHook(() => useSpinnerFrame());
    expect(vi.getTimerCount()).toBe(1);
    act(() => motion.change(true));
    expect(result.current).toBe(SPINNER_STATIC);
    expect(vi.getTimerCount()).toBe(0);
    act(() => motion.change(false));
    expect(result.current).toBe(SPINNER_FRAMES[0]);
    expect(vi.getTimerCount()).toBe(1);
    unmount();
    expect(motion.listenerCount()).toBe(0);
    expect(vi.getTimerCount()).toBe(0);
  });
});
