import { describe, expect, it } from "vitest";
import type { UsageBucket } from "../../api/types";
import { axisTicks, barPercents, formatCount, formatSpan, mergeBuckets, TARGET_BARS } from "./usageSeries";

const HOUR = 3_600_000;

const hourly = (n: number): UsageBucket[] =>
  Array.from({ length: n }, (_, i) => ({
    start: new Date(Date.UTC(2026, 10, 5, i)).toISOString(),
    runs: 1,
    tokens: 100 * (i + 1),
  }));

describe("mergeBuckets", () => {
  it("leaves buckets alone when there are no more than the target", () => {
    const buckets = hourly(6);
    expect(mergeBuckets(buckets, HOUR, 12)).toEqual({ buckets, bucketMs: HOUR });
  });

  it("merges neighbours into wider buckets, adding runs and tokens", () => {
    const merged = mergeBuckets(hourly(4), HOUR, 2);
    expect(merged.bucketMs).toBe(2 * HOUR);
    expect(merged.buckets).toEqual([
      { start: "2026-11-05T00:00:00.000Z", runs: 2, tokens: 300 },
      { start: "2026-11-05T02:00:00.000Z", runs: 2, tokens: 700 },
    ]);
  });

  it("keeps a short last bucket when the count does not divide evenly", () => {
    const merged = mergeBuckets(hourly(5), HOUR, 3);
    expect(merged.buckets.map((b) => b.runs)).toEqual([2, 2, 1]);
    expect(merged.bucketMs).toBe(2 * HOUR);
  });

  it("keeps the totals", () => {
    const before = hourly(28);
    const after = mergeBuckets(before, 6 * HOUR, TARGET_BARS["7d"]);
    const sum = (bs: readonly UsageBucket[], key: "runs" | "tokens") => bs.reduce((n, b) => n + b[key], 0);
    expect(sum(after.buckets, "runs")).toBe(sum(before, "runs"));
    expect(sum(after.buckets, "tokens")).toBe(sum(before, "tokens"));
  });

  it("gives every range fewer, wider bars than the daemon's buckets", () => {
    expect(TARGET_BARS["24h"]).toBeLessThan(24);
    expect(TARGET_BARS["7d"]).toBeLessThan(28);
    expect(TARGET_BARS["30d"]).toBeLessThan(30);
  });
});

describe("axisTicks", () => {
  it("labels the first, last and a few bars between with the clock for a day", () => {
    const ticks = axisTicks(hourly(24), "24h", 4);
    expect(ticks[0]).toEqual({ at: 0, label: "00:00" });
    expect(ticks.map((t) => t.label)).toEqual(["00:00", "08:00", "15:00", "23:00"]);
    expect(ticks.at(-1)?.at).toBe(23);
  });

  it("labels longer ranges with the date", () => {
    const buckets = Array.from({ length: 30 }, (_, i) => ({
      start: new Date(Date.UTC(2026, 9, 11 + i)).toISOString(),
      runs: 0,
      tokens: 0,
    }));
    const ticks = axisTicks(buckets, "30d", 3);
    expect(ticks.map((t) => t.label)).toEqual(["10-11", "10-26", "11-09"]);
  });

  it("has no more ticks than bars and none for no bars", () => {
    expect(axisTicks(hourly(2), "24h", 5)).toHaveLength(2);
    expect(axisTicks([], "24h", 4)).toEqual([]);
  });
});

describe("formatSpan", () => {
  it("shows an hour inside one day as a clock range in UTC", () => {
    expect(formatSpan("2026-11-05T14:00:00.000Z", HOUR)).toBe("2026-11-05 14:00–15:00 UTC");
  });

  it("shows both days when the bucket crosses midnight", () => {
    expect(formatSpan("2026-11-05T18:00:00.000Z", 6 * HOUR)).toBe("2026-11-05 18:00 → 2026-11-06 00:00 UTC");
  });

  it("shows a whole day from midnight to midnight", () => {
    expect(formatSpan("2026-11-05T00:00:00.000Z", 24 * HOUR)).toBe("2026-11-05 00:00 → 2026-11-06 00:00 UTC");
  });
});

describe("formatCount", () => {
  it("writes exact numbers with thousands separators", () => {
    expect(formatCount(0)).toBe("0");
    expect(formatCount(1_234_567)).toBe("1,234,567");
  });
});

describe("barPercents", () => {
  it("scales the largest value to 100 and the rest in proportion", () => {
    expect(barPercents([0, 5, 10])).toEqual([0, 50, 100]);
  });

  it("keeps a small non-zero value visible", () => {
    expect(barPercents([1, 1000])[0]).toBeGreaterThanOrEqual(4);
  });

  it("is all zero when every value is zero or there are none", () => {
    expect(barPercents([0, 0])).toEqual([0, 0]);
    expect(barPercents([])).toEqual([]);
  });
});
