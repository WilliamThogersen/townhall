import { act, renderHook } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { logWarn } from "../../log/report";
import { defaultAvatar, type Avatar } from "./avatar";
import {
  AVATAR_PREFS_KEY,
  avatarFor,
  clearAvatar,
  readAvatarPrefs,
  saveAvatar,
  useAvatar,
} from "./avatarPrefs";

vi.mock("../../log/report", () => ({ logWarn: vi.fn() }));

const custom: Avatar = { head: "tapered", eyes: "cyclops", mouth: "grill", accessory: "crest", color: "info" };

afterEach(() => {
  vi.restoreAllMocks();
  vi.mocked(logWarn).mockClear();
  localStorage.clear();
});

describe("avatar prefs", () => {
  it("falls back to the name-hash default without a saved choice", () => {
    expect(avatarFor("planner")).toEqual(defaultAvatar("planner"));
  });

  it("remembers a saved choice per agent type name", () => {
    expect(saveAvatar("planner", custom)).toBe(true);
    expect(avatarFor("planner")).toEqual(custom);
    expect(avatarFor("coder")).toEqual(defaultAvatar("coder"));
    expect(JSON.parse(localStorage.getItem(AVATAR_PREFS_KEY) ?? "null")).toEqual({ planner: custom });
  });

  it("matches names without regard to case", () => {
    saveAvatar("Planner", custom);
    expect(avatarFor("planner")).toEqual(custom);
  });

  it("clears back to the default", () => {
    saveAvatar("planner", custom);
    clearAvatar("planner");
    expect(avatarFor("planner")).toEqual(defaultAvatar("planner"));
    expect(readAvatarPrefs()).toEqual({});
  });

  it("ignores a saved avatar that is not valid and keeps the valid ones", () => {
    localStorage.setItem(AVATAR_PREFS_KEY, JSON.stringify({ planner: { head: "cube" }, coder: custom }));
    expect(avatarFor("planner")).toEqual(defaultAvatar("planner"));
    expect(avatarFor("coder")).toEqual(custom);
  });

  it("warns and uses defaults when the stored text is not JSON", () => {
    localStorage.setItem(AVATAR_PREFS_KEY, "{nope");
    expect(avatarFor("planner")).toEqual(defaultAvatar("planner"));
    expect(logWarn).toHaveBeenCalledWith("web.avatar_prefs_invalid", expect.stringContaining("avatar"), expect.anything());
  });

  it("reports false instead of throwing when storage refuses the write", () => {
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(saveAvatar("planner", custom)).toBe(false);
    expect(avatarFor("planner")).toEqual(defaultAvatar("planner"));
  });

  it("works when storage cannot be read", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(avatarFor("planner")).toEqual(defaultAvatar("planner"));
  });
});

describe("useAvatar", () => {
  it("re-renders every user of a name when the choice changes", () => {
    const first = renderHook(() => useAvatar("planner"));
    const second = renderHook(() => useAvatar("planner"));
    const other = renderHook(() => useAvatar("coder"));
    expect(first.result.current).toEqual(defaultAvatar("planner"));
    const otherBefore = other.result.current;
    act(() => {
      saveAvatar("planner", custom);
    });
    expect(first.result.current).toEqual(custom);
    expect(second.result.current).toEqual(custom);
    expect(other.result.current).toBe(otherBefore);
  });

  it("returns the same object between renders when nothing changed", () => {
    const { result, rerender } = renderHook(() => useAvatar("planner"));
    const before = result.current;
    rerender();
    expect(result.current).toBe(before);
  });
});
