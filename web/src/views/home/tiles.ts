import { compactNumber } from "./format";
import { METRICS, type Metric } from "./metrics";
import { dailyValues, KEPT_PERIODS, periodDates, periodYear, sum, type DaysByDate } from "./shipped";

export type DeltaDir = "up" | "down" | "flat";

export interface Delta {
  readonly dir: DeltaDir;
  readonly text: string;
}

export interface TileModel {
  readonly metric: Metric;
  readonly value: string;
  readonly delta: Delta;
}

const MINUS = "−";
const PLUS_MINUS = "±";

function direction(diff: number): DeltaDir {
  if (diff > 0) return "up";
  if (diff < 0) return "down";
  return "flat";
}

const SIGNS: Readonly<Record<DeltaDir, string>> = { up: "+", down: MINUS, flat: PLUS_MINUS };

export function deltaOf(now: number, before: number | null, versus: number): Delta {
  if (before === null) return { dir: "flat", text: `no ${versus} data` };
  const dir = direction(now - before);
  return { dir, text: `${SIGNS[dir]}${compactNumber(Math.abs(now - before))} vs ${versus}` };
}

function periodTotal(today: Date, days: DaysByDate, period: number, metric: Metric): number | null {
  if (period >= KEPT_PERIODS) return null;
  return sum(dailyValues(periodDates(today, period), days, metric.key));
}

export function tileModels(today: Date, days: DaysByDate, period: number): readonly TileModel[] {
  const versus = periodYear(today, period + 1);
  return METRICS.map((metric) => {
    const now = periodTotal(today, days, period, metric) ?? 0;
    const before = periodTotal(today, days, period + 1, metric);
    return { metric, value: `${metric.sign}${compactNumber(now)}`, delta: deltaOf(now, before, versus) };
  });
}
