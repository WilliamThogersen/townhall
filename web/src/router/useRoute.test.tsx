import { act, renderHook } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { currentUrl, navigate, subscribe } from "./history";
import { useRoute } from "./useRoute";

beforeEach(() => window.history.replaceState(null, "", "/"));

describe("navigate", () => {
  it("pushes a history entry and notifies subscribers", () => {
    const listener = vi.fn();
    const stop = subscribe(listener);
    const before = window.history.length;
    navigate("/repos");
    expect(window.location.pathname).toBe("/repos");
    expect(window.history.length).toBe(before + 1);
    expect(listener).toHaveBeenCalledOnce();
    stop();
    navigate("/settings");
    expect(listener).toHaveBeenCalledOnce();
  });

  it("replaces the entry when asked", () => {
    const before = window.history.length;
    navigate("/search?q=x", { replace: true });
    expect(currentUrl()).toBe("/search?q=x");
    expect(window.history.length).toBe(before);
  });

  it("does nothing when the url is already current", () => {
    navigate("/repos");
    const listener = vi.fn();
    const stop = subscribe(listener);
    const before = window.history.length;
    navigate("/repos");
    expect(window.history.length).toBe(before);
    expect(listener).not.toHaveBeenCalled();
    stop();
  });
});

describe("useRoute", () => {
  it("reads the current location", () => {
    window.history.replaceState(null, "", "/c/abc?tab=files");
    const { result } = renderHook(() => useRoute());
    expect(result.current).toEqual({
      name: "conversation",
      path: "/c/abc",
      params: { id: "abc" },
      query: { tab: "files" },
    });
  });

  it("follows navigate", () => {
    const { result } = renderHook(() => useRoute());
    expect(result.current.name).toBe("home");
    act(() => navigate("/townhall?c=abc"));
    expect(result.current.name).toBe("control");
    expect(result.current.query).toEqual({ c: "abc" });
  });

  it("follows the back and forward buttons", () => {
    const { result } = renderHook(() => useRoute());
    act(() => {
      window.history.pushState(null, "", "/settings");
      window.dispatchEvent(new PopStateEvent("popstate"));
    });
    expect(result.current.name).toBe("settings");
  });

  it("keeps the same route object while the url is unchanged", () => {
    const { result, rerender } = renderHook(() => useRoute());
    const first = result.current;
    rerender();
    expect(result.current).toBe(first);
  });
});
