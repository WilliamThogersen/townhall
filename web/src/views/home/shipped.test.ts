import { describe, expect, it } from "vitest";
import type { ShippedDay } from "../../api/types";
import {
  dateKey,
  hasShipped,
  indexDays,
  periodDates,
  periodsPresent,
  periodYear,
  dailyValues,
  sum,
} from "./shipped";

const TODAY = new Date(2026, 10, 4);

const day = (date: string, over: Partial<ShippedDay> = {}): ShippedDay => ({
  date,
  fixes: 0,
  features: 0,
  lines: 0,
  agents: 0,
  ...over,
});

describe("dateKey", () => {
  it("formats the local date", () => {
    expect(dateKey(new Date(2026, 0, 5))).toBe("2026-01-05");
    expect(dateKey(new Date(2026, 10, 14))).toBe("2026-11-14");
  });
});

describe("periodDates", () => {
  it("covers 53 week columns from a monday up to today", () => {
    const dates = periodDates(TODAY, 0);
    expect(dates.at(-1)).toEqual(TODAY);
    expect(dates[0]?.getDay()).toBe(1);
    expect(dates.length).toBe(52 * 7 + 3);
    expect(Math.ceil(dates.length / 7)).toBe(53);
  });

  it("ends on the same day one year earlier for the previous period", () => {
    const dates = periodDates(TODAY, 1);
    expect(dateKey(dates.at(-1)!)).toBe("2025-11-04");
    expect(dates[0]?.getDay()).toBe(1);
  });

  it("has 52 full weeks plus today when today is a monday", () => {
    const monday = new Date(2026, 10, 2);
    expect(periodDates(monday, 0).length).toBe(52 * 7 + 1);
  });

  it("labels a period with the year it ends in", () => {
    expect(periodYear(TODAY, 0)).toBe(2026);
    expect(periodYear(TODAY, 1)).toBe(2025);
  });
});

describe("values", () => {
  it("reads one value per date, zero where nothing shipped", () => {
    const days = indexDays([day("2026-11-03", { fixes: 2 }), day("2026-11-04", { fixes: 1, lines: 40 })]);
    const dates = [new Date(2026, 10, 2), new Date(2026, 10, 3), new Date(2026, 10, 4)];
    expect(dailyValues(dates, days, "fixes")).toEqual([0, 2, 1]);
    expect(sum(dailyValues(dates, days, "lines"))).toBe(40);
  });
});

describe("hasShipped", () => {
  it("is false with no days or only zero days", () => {
    expect(hasShipped({ days: [] })).toBe(false);
    expect(hasShipped({ days: [day("2026-11-01")] })).toBe(false);
  });

  it("is true once any number is above zero", () => {
    expect(hasShipped({ days: [day("2026-11-01", { agents: 1 })] })).toBe(true);
  });
});

describe("periodsPresent", () => {
  it("always has the current period", () => {
    expect(periodsPresent(TODAY, indexDays([]))).toEqual([0]);
  });

  it("adds the previous period when it has activity", () => {
    const days = indexDays([day("2025-06-10", { features: 1 })]);
    expect(periodsPresent(TODAY, days)).toEqual([0, 1]);
  });

  it("leaves the previous period out when its days are all zero", () => {
    const days = indexDays([day("2025-06-10")]);
    expect(periodsPresent(TODAY, days)).toEqual([0]);
  });
});
