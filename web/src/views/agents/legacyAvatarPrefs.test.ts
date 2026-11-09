import { afterEach, describe, expect, it, vi } from "vitest";
import { logWarn } from "../../log/report";
import { dropLegacyAvatarPrefs, LEGACY_AVATAR_PREFS_KEY } from "./legacyAvatarPrefs";

vi.mock("../../log/report", () => ({ logWarn: vi.fn() }));

afterEach(() => {
  vi.restoreAllMocks();
  vi.mocked(logWarn).mockClear();
  localStorage.clear();
});

describe("dropLegacyAvatarPrefs", () => {
  it("removes the browser-only avatar choices and leaves other keys alone", () => {
    localStorage.setItem(LEGACY_AVATAR_PREFS_KEY, '{"planner":{"head":"round"}}');
    localStorage.setItem("th-other", "keep");
    dropLegacyAvatarPrefs();
    expect(localStorage.getItem(LEGACY_AVATAR_PREFS_KEY)).toBeNull();
    expect(localStorage.getItem("th-other")).toBe("keep");
  });

  it("does nothing when there is nothing stored", () => {
    dropLegacyAvatarPrefs();
    expect(logWarn).not.toHaveBeenCalled();
  });

  it("uses the key the old store wrote", () => {
    expect(LEGACY_AVATAR_PREFS_KEY).toBe("th-agent-avatars");
  });

  it("warns and carries on when the browser refuses", () => {
    vi.spyOn(Storage.prototype, "removeItem").mockImplementation(() => {
      throw new Error("denied");
    });
    expect(() => dropLegacyAvatarPrefs()).not.toThrow();
    expect(logWarn).toHaveBeenCalledWith("web.avatar_prefs_not_dropped", expect.any(String), expect.any(Error));
  });
});
