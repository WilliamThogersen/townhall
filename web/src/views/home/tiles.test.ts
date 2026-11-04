import { describe, expect, it } from "vitest";
import type { ShippedDay } from "../../api/types";
import { indexDays } from "./shipped";
import { deltaOf, tileModels } from "./tiles";

const TODAY = new Date(2026, 10, 4);

const day = (date: string, over: Partial<ShippedDay>): ShippedDay => ({
  date,
  fixes: 0,
  features: 0,
  lines: 0,
  agents: 0,
  ...over,
});

describe("deltaOf", () => {
  it("is green and signed when up", () => {
    expect(deltaOf(30, 12, 2025)).toEqual({ dir: "up", text: "+18 vs 2025" });
  });

  it("uses a minus sign when down", () => {
    expect(deltaOf(4, 10, 2025)).toEqual({ dir: "down", text: "−6 vs 2025" });
  });

  it("is flat when equal", () => {
    expect(deltaOf(5, 5, 2025)).toEqual({ dir: "flat", text: "±0 vs 2025" });
  });

  it("compacts big differences", () => {
    expect(deltaOf(30_000, 1_600, 2025).text).toBe("+28.4K vs 2025");
  });

  it("says when the earlier year is not kept", () => {
    expect(deltaOf(5, null, 2024)).toEqual({ dir: "flat", text: "no 2024 data" });
  });
});

describe("tileModels", () => {
  const days = indexDays([
    day("2026-11-04", { fixes: 2, lines: 12_000, agents: 3 }),
    day("2026-01-10", { fixes: 1, features: 2 }),
    day("2025-10-01", { fixes: 1, lines: 400 }),
  ]);

  it("totals each metric over the period and compares with the year before", () => {
    const tiles = tileModels(TODAY, days, 0);
    expect(tiles.map((t) => [t.metric.label, t.value, t.delta.text])).toEqual([
      ["bug fixes", "3", "+2 vs 2025"],
      ["features", "2", "+2 vs 2025"],
      ["lines shipped", "+12.0K", "+11.6K vs 2025"],
      ["agents run", "3", "+3 vs 2025"],
    ]);
  });

  it("has no comparison for the oldest kept period", () => {
    const tiles = tileModels(TODAY, days, 1);
    expect(tiles[0]?.value).toBe("1");
    expect(tiles[0]?.delta.text).toBe("no 2024 data");
    expect(tiles[2]?.value).toBe("+400");
  });
});
