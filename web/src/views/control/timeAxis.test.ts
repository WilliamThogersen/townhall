import { describe, expect, it } from "vitest";
import {
  durationLabel,
  MAX_TICKS,
  NOW_AT,
  percentAt,
  spanPercents,
  startOfDay,
  tickLabel,
  ticksOf,
  timeAxis,
  windowLabel,
} from "./timeAxis";

const local = (h: number, m = 0, day = 4) => new Date(2026, 10, day, h, m).getTime();
const MINUTE = 60_000;

describe("startOfDay", () => {
  it("is local midnight of the same day", () => {
    expect(startOfDay(local(21, 29))).toBe(local(0));
  });
});

describe("timeAxis", () => {
  it("floors the start to a tick and keeps now near three quarters", () => {
    const axis = timeAxis(local(19, 6), local(21, 29));
    expect(axis.step).toBe(30 * MINUTE);
    expect(axis.start).toBe(local(19));
    expect(axis.ticks).toBeLessThanOrEqual(MAX_TICKS);
    expect(axis.end).toBe(axis.start + axis.ticks * axis.step);
    const nowAt = (axis.now - axis.start) / (axis.end - axis.start);
    expect(nowAt).toBeLessThanOrEqual(NOW_AT);
    expect(nowAt).toBeGreaterThan(0.5);
    expect(axis.today).toBe(true);
  });

  it("uses small ticks for a short window and a minimum span", () => {
    const axis = timeAxis(local(10, 41), local(10, 42));
    expect(axis.step).toBe(5 * MINUTE);
    expect(axis.start).toBe(local(10, 30));
    expect(axis.end - axis.start).toBeGreaterThanOrEqual(10 * MINUTE);
  });

  it("goes back past midnight for an older live item and says so", () => {
    const axis = timeAxis(local(14, 0, 2), local(9, 0));
    expect(axis.today).toBe(false);
    expect(axis.start).toBeLessThanOrEqual(local(14, 0, 2));
    expect(axis.ticks).toBeLessThanOrEqual(MAX_TICKS);
    expect(windowLabel(axis)).toBe("since 2 nov");
  });

  it("treats a start after now as now", () => {
    const axis = timeAxis(local(12), local(11));
    expect(axis.start).toBeLessThanOrEqual(local(11));
    expect(windowLabel(axis)).toBe("today");
  });
});

describe("percentAt and spanPercents", () => {
  const axis = { start: local(10), end: local(11), step: 10 * MINUTE, ticks: 6, now: local(10, 45), today: true };

  it("places a time on the axis, clamped to it", () => {
    expect(percentAt(axis, local(10, 30))).toBe(50);
    expect(percentAt(axis, local(9))).toBe(0);
    expect(percentAt(axis, local(12))).toBe(100);
  });

  it("gives start and length, never negative", () => {
    expect(spanPercents(axis, local(10, 15), local(10, 45))).toEqual({ start: 25, len: 50 });
    expect(spanPercents(axis, local(9), local(10, 6))).toEqual({ start: 0, len: 10 });
    expect(spanPercents(axis, local(10, 30), local(10, 15))).toEqual({ start: 50, len: 0 });
  });
});

describe("ticks", () => {
  it("lists one tick per step from the start, without the end", () => {
    const axis = timeAxis(local(19, 6), local(21, 29));
    const ticks = ticksOf(axis);
    expect(ticks).toHaveLength(axis.ticks);
    expect(ticks[0]).toEqual({ at: 0, label: "19:00" });
    expect(ticks[1]?.label).toBe("19:30");
  });

  it("labels day steps with the date", () => {
    expect(tickLabel(local(0, 0, 2), 24 * 60 * MINUTE)).toBe("2 nov");
    expect(tickLabel(local(9, 5), 5 * MINUTE)).toBe("09:05");
  });
});

describe("durationLabel", () => {
  it.each([
    [0, "0s"],
    [54_000, "54s"],
    [87_000, "1m"],
    [16 * MINUTE, "16m"],
    [60 * MINUTE, "1h"],
    [72 * MINUTE, "1h 12m"],
    [26 * 60 * MINUTE, "1d 2h"],
    [48 * 60 * MINUTE, "2d"],
    [-5, "0s"],
  ])("%d ms is %s", (ms, label) => {
    expect(durationLabel(ms)).toBe(label);
  });
});
