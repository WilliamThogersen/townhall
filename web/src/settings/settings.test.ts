import { describe, expect, it } from "vitest";
import {
  AGENTS_AT_ONCE_MAX,
  AGENTS_AT_ONCE_MIN,
  DEFAULT_SETTINGS,
  parseSettings,
  type Settings,
} from "./settings";

const custom: Settings = {
  permissionMode: "plan",
  workWithoutAsking: true,
  agentsAtOnce: 12,
  repoUnclear: "guess",
  sessionPanelOpen: false,
  notifyWhen: "everything",
};

describe("DEFAULT_SETTINGS", () => {
  it("matches the design defaults", () => {
    expect(DEFAULT_SETTINGS).toEqual({
      permissionMode: "acceptEdits",
      workWithoutAsking: false,
      agentsAtOnce: 6,
      repoUnclear: "ask",
      sessionPanelOpen: true,
      notifyWhen: "needsYou",
    });
  });

  it("is frozen", () => {
    expect(Object.isFrozen(DEFAULT_SETTINGS)).toBe(true);
  });

  it("keeps agents at once between 1 and 50", () => {
    expect(AGENTS_AT_ONCE_MIN).toBe(1);
    expect(AGENTS_AT_ONCE_MAX).toBe(50);
  });
});

describe("parseSettings", () => {
  it("keeps every valid field", () => {
    expect(parseSettings({ ...custom })).toEqual(custom);
  });

  it("accepts every permission mode", () => {
    const modes = ["default", "acceptEdits", "plan", "auto", "bypassPermissions"] as const;
    modes.forEach((permissionMode) => {
      expect(parseSettings({ permissionMode }).permissionMode).toBe(permissionMode);
    });
  });

  it("falls back to the defaults for anything that is not an object", () => {
    [null, undefined, 3, "x", true, [1, 2]].forEach((raw) => {
      expect(parseSettings(raw)).toEqual(DEFAULT_SETTINGS);
    });
  });

  it("falls back per field for missing or wrong values", () => {
    const parsed = parseSettings({
      permissionMode: "yolo",
      workWithoutAsking: "yes",
      agentsAtOnce: 12,
      repoUnclear: 1,
      notifyWhen: null,
    });
    expect(parsed).toEqual({ ...DEFAULT_SETTINGS, agentsAtOnce: 12 });
  });

  it("rejects agents at once outside 1 to 50 or not a whole number", () => {
    [0, 51, -3, 2.5, Number.NaN, Number.POSITIVE_INFINITY, "6"].forEach((agentsAtOnce) => {
      expect(parseSettings({ agentsAtOnce }).agentsAtOnce).toBe(DEFAULT_SETTINGS.agentsAtOnce);
    });
    expect(parseSettings({ agentsAtOnce: 1 }).agentsAtOnce).toBe(1);
    expect(parseSettings({ agentsAtOnce: 50 }).agentsAtOnce).toBe(50);
  });

  it("drops unknown fields", () => {
    expect(parseSettings({ ...custom, theme: "light" })).toEqual(custom);
  });

  it("returns a frozen object", () => {
    expect(Object.isFrozen(parseSettings(custom))).toBe(true);
  });
});
