import type { ShippedDay, Stats } from "../../api/types";
import { weekdayIndex } from "./format";
import { METRICS, type MetricKey } from "./metrics";

export const KEPT_PERIODS = 2;
const WEEKS_BEFORE_THIS_ONE = 52;
const DAYS_PER_WEEK = 7;

export type DaysByDate = ReadonlyMap<string, ShippedDay>;

const pad = (n: number): string => String(n).padStart(2, "0");

export function dateKey(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

export function periodEnd(today: Date, period: number): Date {
  return new Date(today.getFullYear() - period, today.getMonth(), today.getDate());
}

export function periodYear(today: Date, period: number): number {
  return periodEnd(today, period).getFullYear();
}

export function periodDates(today: Date, period: number): readonly Date[] {
  const end = periodEnd(today, period);
  const count = weekdayIndex(end) + WEEKS_BEFORE_THIS_ONE * DAYS_PER_WEEK + 1;
  const first = end.getDate() - count + 1;
  return Array.from({ length: count }, (_, i) => new Date(end.getFullYear(), end.getMonth(), first + i));
}

export function indexDays(days: readonly ShippedDay[]): DaysByDate {
  return new Map(days.map((day) => [day.date, day]));
}

export function dailyValues(dates: readonly Date[], days: DaysByDate, key: MetricKey): readonly number[] {
  return dates.map((date) => days.get(dateKey(date))?.[key] ?? 0);
}

export const sum = (values: readonly number[]): number => values.reduce((total, v) => total + v, 0);

const isActive = (day: ShippedDay | undefined): boolean =>
  day !== undefined && METRICS.some((metric) => day[metric.key] > 0);

export function hasShipped(stats: Stats): boolean {
  return stats.days.some(isActive);
}

export function periodsPresent(today: Date, days: DaysByDate): readonly number[] {
  const earlier = Array.from({ length: KEPT_PERIODS - 1 }, (_, i) => i + 1);
  const active = earlier.filter((period) =>
    periodDates(today, period).some((date) => isActive(days.get(dateKey(date)))),
  );
  return [0, ...active];
}
