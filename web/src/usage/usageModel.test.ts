import { describe, expect, it } from "vitest";
import { aUsage } from "../api/fixtures";
import type { UsageWindow } from "../api/types";
import {
  footLabel,
  isStale,
  meterLabel,
  projectRunOut,
  resetLabel,
  resetSentence,
  resetTimeLabel,
  runOutSentence,
  staleReason,
  usageLevel,
  windowView,
} from "./usageModel";

const HOUR = 3_600_000;
const at = (day: number, hour: number, minute = 0) => new Date(2026, 10, day, hour, minute).getTime();
const NOW = at(5, 12, 39);

const window = (usedPercent: number, resetsAt: number | null, ratePerHour: number | null = null): UsageWindow => ({
  usedPercent,
  resetsAt,
  ratePerHour,
});

describe("reset labels", () => {
  it("counts down in the largest two units", () => {
    expect(resetLabel(NOW + 150 * 60_000, NOW)).toBe("2h 30m");
    expect(resetLabel(NOW + 45 * 60_000, NOW)).toBe("45m");
    expect(resetLabel(at(11, 8), NOW)).toBe("5d 19h");
    expect(resetLabel(NOW + 24 * HOUR + 25 * 60_000, NOW)).toBe("1d 0h");
    expect(resetLabel(NOW + 30_000, NOW)).toBe("<1m");
    expect(resetLabel(NOW - 1, NOW)).toBe("now");
    expect(resetLabel(null, NOW)).toBe("");
  });

  it("gives the local time, with the weekday when it is not today", () => {
    expect(resetTimeLabel(at(5, 15, 10), NOW)).toBe("15:10");
    expect(resetTimeLabel(at(5, 9, 5), NOW)).toBe("09:05");
    expect(resetTimeLabel(at(11, 8), NOW)).toBe("Wed 08:00");
    expect(resetTimeLabel(null, NOW)).toBe("");
  });

  it("says when it resets, or that the reset is due", () => {
    expect(resetSentence(at(5, 15, 10), NOW)).toBe("resets 15:10 (in 2h 31m)");
    expect(resetSentence(NOW, NOW)).toBe("reset due");
    expect(resetSentence(null, NOW)).toBe("");
  });
});

describe("running out", () => {
  it("projects the hour usage reaches 100% at the current rate", () => {
    const week = window(54, at(11, 8), 2);
    expect(projectRunOut(week, NOW)).toEqual({ kind: "runsOut", at: NOW + 23 * HOUR });
  });

  it("lasts when the reset comes first, the rate is zero or unknown, or the reset is unknown", () => {
    expect(projectRunOut(window(30, NOW + 2 * HOUR, 12), NOW)).toEqual({ kind: "lasts" });
    expect(projectRunOut(window(30, NOW + 9 * HOUR, 0), NOW)).toEqual({ kind: "lasts" });
    expect(projectRunOut(window(30, NOW + 9 * HOUR, null), NOW)).toEqual({ kind: "lasts" });
    expect(projectRunOut(window(30, null, 50), NOW)).toEqual({ kind: "lasts" });
  });

  it("reports a full window as reached", () => {
    expect(projectRunOut(window(100, NOW + HOUR, 5), NOW)).toEqual({ kind: "reached" });
  });

  it("words each outcome", () => {
    expect(runOutSentence({ kind: "lasts" }, NOW)).toBe("lasts until reset");
    expect(runOutSentence({ kind: "reached" }, NOW)).toBe("limit reached");
    expect(runOutSentence({ kind: "runsOut", at: NOW + 23 * HOUR }, NOW)).toBe("runs out ~Fri 11:39 (in 23h 0m)");
    expect(runOutSentence({ kind: "runsOut", at: NOW }, NOW)).toBe("runs out ~12:39");
  });
});

describe("window view", () => {
  it("is the popover row of the design", () => {
    expect(windowView("5 hours", window(30, at(5, 15, 10), 1), NOW, NOW)).toEqual({
      name: "5 hours",
      used: "30% used",
      reset: "resets 15:10 (in 2h 31m)",
      runOut: "lasts until reset",
      runsOut: false,
    });
  });

  it("flags a window that runs out before it resets", () => {
    const view = windowView("Week", window(54, at(11, 8), 2), NOW, NOW);
    expect(view).toMatchObject({ reset: "resets Wed 08:00 (in 5d 19h)", runOut: "runs out ~Fri 11:39 (in 23h 0m)", runsOut: true });
  });

  it("projects from the reading time, so an old reading does not slide", () => {
    const readAt = NOW - 2 * HOUR;
    const view = windowView("Week", window(54, at(11, 8), 2), readAt, NOW);
    expect(view.runOut).toBe("runs out ~Fri 09:39 (in 21h 0m)");
  });

  it("shows a window whose reset has passed as empty until the next reading", () => {
    expect(windowView("5 hours", window(80, NOW - 60_000, 3), NOW - HOUR, NOW)).toEqual({
      name: "5 hours",
      used: "0% used",
      reset: "reset since last update",
      runOut: null,
      runsOut: false,
    });
  });

  it("copes with an unknown reset", () => {
    const view = windowView("Week", window(54, null), NOW, NOW);
    expect(view).toMatchObject({ used: "54% used", reset: "reset time unknown", runOut: "lasts until reset" });
  });
});

describe("meter text", () => {
  it("colours by how full the window is", () => {
    expect([0, 59.9, 60, 79.9, 80, 100].map(usageLevel)).toEqual(["ok", "ok", "warn", "warn", "hot", "hot"]);
  });

  it("labels a meter with its name, use and reset", () => {
    expect(meterLabel("5h", window(29.6, at(5, 15, 10)), NOW)).toBe("5h 30% used · resets 15:10 (in 2h 31m)");
    expect(meterLabel("wk", window(54, null), NOW)).toBe("wk 54% used");
    expect(meterLabel("5h", window(80, NOW - 1), NOW)).toBe("5h 0% used");
  });
});

describe("freshness", () => {
  it("is stale when a refresh failed but numbers are still shown", () => {
    expect(isStale(aUsage())).toBe(false);
    expect(isStale(aUsage({ error: "Offline" }))).toBe(true);
    expect(isStale(aUsage({ session: null, weekly: null, error: "x" }))).toBe(false);
    expect(isStale(null)).toBe(false);
  });

  it("words the foot of the popover", () => {
    const updatedAt = at(5, 12, 39);
    expect(footLabel(aUsage({ updatedAt }))).toBe("Updated 12:39");
    expect(footLabel(aUsage({ updatedAt, error: "Usage unavailable (offline?)" }))).toBe("Stale · Updated 12:39");
  });

  it("explains why the numbers are old", () => {
    expect(staleReason(null)).toBe("");
    expect(staleReason("Sign in to Claude Code to see usage")).toBe("Sign in to Claude Code to refresh");
    expect(staleReason("Usage rate limited, retrying later")).toBe("Rate limited, retrying later");
    expect(staleReason("Usage unavailable (offline?)")).toBe("Offline");
    expect(staleReason("Not refreshed yet")).toBe("Not refreshed yet");
    expect(staleReason("Usage unavailable (HTTP 500)")).toBe("Usage unavailable (HTTP 500)");
  });
});
