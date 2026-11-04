import { describe, expect, it } from "vitest";
import { compactNumber, countLabel, dayLabel, monthLabel } from "./format";
import { METRICS } from "./metrics";

const [fixes, , lines] = METRICS;

describe("compactNumber", () => {
  it("keeps numbers up to 9,999 whole", () => {
    expect(compactNumber(0)).toBe("0");
    expect(compactNumber(265)).toBe("265");
    expect(compactNumber(9999)).toBe("9999");
  });

  it("compacts past 9,999 to one decimal", () => {
    expect(compactNumber(10_000)).toBe("10.0K");
    expect(compactNumber(18_412)).toBe("18.4K");
    expect(compactNumber(999_949)).toBe("999.9K");
  });

  it("switches to millions before 1000.0K", () => {
    expect(compactNumber(999_950)).toBe("1.0M");
    expect(compactNumber(2_340_000)).toBe("2.3M");
  });
});

describe("countLabel", () => {
  it("uses the singular for one", () => {
    expect(countLabel(1, fixes!)).toBe("1 bug fix");
    expect(countLabel(1, lines!)).toBe("1 line");
  });

  it("uses the plural otherwise and compacts", () => {
    expect(countLabel(0, fixes!)).toBe("0 bug fixes");
    expect(countLabel(3, fixes!)).toBe("3 bug fixes");
    expect(countLabel(18_412, lines!)).toBe("18.4K lines");
  });
});

describe("dates", () => {
  it("names a day like tue 14 oct", () => {
    expect(dayLabel(new Date(2025, 9, 14))).toBe("tue 14 oct");
    expect(dayLabel(new Date(2026, 10, 1))).toBe("sun 1 nov");
  });

  it("names a month with its year", () => {
    expect(monthLabel(new Date(2026, 0, 31))).toBe("jan 2026");
  });
});
