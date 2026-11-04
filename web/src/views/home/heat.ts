import { compactNumber, countLabel, dayLabel, monthLabel, MONTHS } from "./format";
import type { Metric } from "./metrics";
import { sum } from "./shipped";

export interface HeatCell {
  readonly index: number;
  readonly column: number;
  readonly row: number;
  readonly level: number;
  readonly tip: string;
}

export interface MonthMark {
  readonly column: number;
  readonly span: number;
  readonly name: string;
}

export interface MonthTotal {
  readonly label: string;
  readonly total: string;
}

export interface HeatModel {
  readonly cells: readonly HeatCell[];
  readonly months: readonly MonthMark[];
  readonly caption: string;
  readonly tableCaption: string;
  readonly monthTotals: readonly MonthTotal[];
}

export interface HeatInput {
  readonly dates: readonly Date[];
  readonly values: readonly number[];
  readonly metric: Metric;
  readonly when: string;
}

const LEVELS = 4;
const DAYS_PER_WEEK = 7;
const FIRST_COLUMN = 2;
const FIRST_ROW = 2;
const LAST_COLUMN = 54;
const MONTH_SPAN = 4;

export function heatLevel(value: number, top: number): number {
  if (value <= 0 || top <= 0) return 0;
  return Math.min(LEVELS, Math.ceil((value / top) * LEVELS));
}

export function periodPhrase(period: number, year: number): string {
  return period === 0 ? "in the last 12 months" : `in ${year}`;
}

const columnOf = (index: number): number => Math.floor(index / DAYS_PER_WEEK) + FIRST_COLUMN;

function monthMarks(dates: readonly Date[]): readonly MonthMark[] {
  const mondays = dates
    .map((date, index) => ({ date, column: columnOf(index), monday: index % DAYS_PER_WEEK === 0 }))
    .filter((d) => d.monday && d.column < LAST_COLUMN - 1);
  return mondays
    .filter((d, i) => i === 0 || mondays[i - 1]?.date.getMonth() !== d.date.getMonth())
    .map((d) => ({
      column: d.column,
      span: Math.min(MONTH_SPAN, LAST_COLUMN + 1 - d.column),
      name: MONTHS[d.date.getMonth()] ?? "",
    }));
}

function monthTotals(dates: readonly Date[], values: readonly number[]): readonly MonthTotal[] {
  const totals = dates.reduce<ReadonlyMap<string, number>>((acc, date, i) => {
    const label = monthLabel(date);
    return new Map(acc).set(label, (acc.get(label) ?? 0) + (values[i] ?? 0));
  }, new Map());
  return [...totals].map(([label, total]) => ({ label, total: compactNumber(total) }));
}

export function heatModel({ dates, values, metric, when }: HeatInput): HeatModel {
  const top = Math.max(0, ...values);
  const cells = dates.map((date, index) => {
    const value = values[index] ?? 0;
    return {
      index,
      column: columnOf(index),
      row: (index % DAYS_PER_WEEK) + FIRST_ROW,
      level: heatLevel(value, top),
      tip: `${countLabel(value, metric)} · ${dayLabel(date)}`,
    };
  });
  return {
    cells,
    months: monthMarks(dates),
    caption: `${countLabel(sum(values), metric)} ${metric.verb} ${when}`,
    tableCaption: `${metric.many} per month`,
    monthTotals: monthTotals(dates, values),
  };
}
