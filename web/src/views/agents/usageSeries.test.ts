import { describe, expect, it } from "vitest";
import { barPercents, formatCount, formatSpan } from "./usageSeries";

const HOUR = 3_600_000;

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
