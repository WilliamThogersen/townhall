import type { Metric } from "./metrics";

const COMPACT_FROM = 10_000;
const MILLIONS_FROM = 999_950;

export const MONTHS = ["jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec"] as const;
export const WEEKDAYS = ["mon", "tue", "wed", "thu", "fri", "sat", "sun"] as const;

export function compactNumber(n: number): string {
  if (n < COMPACT_FROM) return String(n);
  if (n < MILLIONS_FROM) return `${(n / 1_000).toFixed(1)}K`;
  return `${(n / 1_000_000).toFixed(1)}M`;
}

export function countLabel(n: number, metric: Metric): string {
  return `${compactNumber(n)} ${n === 1 ? metric.one : metric.many}`;
}

export function weekdayIndex(date: Date): number {
  return (date.getDay() + 6) % 7;
}

export function dayLabel(date: Date): string {
  return `${WEEKDAYS[weekdayIndex(date)]} ${date.getDate()} ${MONTHS[date.getMonth()]}`;
}

export function monthLabel(date: Date): string {
  return `${MONTHS[date.getMonth()]} ${date.getFullYear()}`;
}
