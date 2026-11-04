import { describe, expect, it } from "vitest";
import { heatLevel, heatModel, periodPhrase } from "./heat";
import { METRICS } from "./metrics";
import { periodDates } from "./shipped";

const TODAY = new Date(2026, 10, 4);
const fixes = METRICS[0]!;
const lines = METRICS[2]!;

describe("heatLevel", () => {
  it("is 0 for nothing and scales to 4 against the busiest day", () => {
    expect(heatLevel(0, 8)).toBe(0);
    expect(heatLevel(1, 8)).toBe(1);
    expect(heatLevel(3, 8)).toBe(2);
    expect(heatLevel(5, 8)).toBe(3);
    expect(heatLevel(8, 8)).toBe(4);
  });

  it("is 0 when the busiest day is 0", () => {
    expect(heatLevel(0, 0)).toBe(0);
  });
});

describe("periodPhrase", () => {
  it("says the last 12 months for the current period and the year otherwise", () => {
    expect(periodPhrase(0, 2026)).toBe("in the last 12 months");
    expect(periodPhrase(1, 2025)).toBe("in 2025");
  });
});

describe("heatModel", () => {
  const dates = periodDates(TODAY, 0);
  const values = dates.map((_, i) => (i === dates.length - 1 ? 3 : i === 0 ? 1 : 0));
  const model = heatModel({ dates, values, metric: fixes, when: "in the last 12 months" });

  it("places days in week columns, monday first, after the label column and row", () => {
    expect(model.cells[0]).toMatchObject({ column: 2, row: 2, level: 2 });
    expect(model.cells[7]).toMatchObject({ column: 3, row: 2, level: 0 });
    expect(model.cells.at(-1)).toMatchObject({ column: 54, row: 4, level: 4 });
  });

  it("writes the hover text for each day", () => {
    expect(model.cells.at(-1)?.tip).toBe("3 bug fixes · wed 4 nov");
    expect(model.cells[0]?.tip).toBe("1 bug fix · mon 3 nov");
  });

  it("labels each month at its first monday column, not in the last column", () => {
    expect(model.months[0]).toEqual({ column: 2, span: 4, name: "nov" });
    expect(model.months[1]).toEqual({ column: 6, span: 4, name: "dec" });
    expect(model.months.every((m) => m.column < 53 && m.column + m.span <= 55)).toBe(true);
    expect(model.months.map((m) => m.name)).toContain("oct");
  });

  it("captions the total for the period", () => {
    expect(model.caption).toBe("4 bug fixes shipped in the last 12 months");
  });

  it("lists monthly totals for screen readers", () => {
    expect(model.tableCaption).toBe("bug fixes per month");
    expect(model.monthTotals[0]).toEqual({ label: "nov 2025", total: "1" });
    expect(model.monthTotals.at(-1)).toEqual({ label: "nov 2026", total: "3" });
    expect(model.monthTotals).toHaveLength(13);
  });

  it("compacts lines", () => {
    const big = heatModel({ dates, values: dates.map(() => 100), metric: lines, when: "in 2025" });
    expect(big.caption).toBe("36.7K lines shipped in 2025");
  });
});
