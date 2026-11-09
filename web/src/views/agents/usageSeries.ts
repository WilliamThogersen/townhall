import type { UsageBucket, UsageRange } from "../../api/types";

const MIN_VISIBLE_PERCENT = 4;

export const TARGET_BARS: Readonly<Record<UsageRange, number>> = { "24h": 12, "7d": 14, "30d": 15 };

const pad = (n: number) => String(n).padStart(2, "0");

const dayOf = (date: Date) => `${date.getUTCFullYear()}-${pad(date.getUTCMonth() + 1)}-${pad(date.getUTCDate())}`;
const clockOf = (date: Date) => `${pad(date.getUTCHours())}:${pad(date.getUTCMinutes())}`;

export function formatSpan(startIso: string, bucketMs: number): string {
  const start = new Date(startIso);
  const end = new Date(start.getTime() + bucketMs);
  if (dayOf(start) === dayOf(end)) return `${dayOf(start)} ${clockOf(start)}–${clockOf(end)} UTC`;
  return `${dayOf(start)} ${clockOf(start)} → ${dayOf(end)} ${clockOf(end)} UTC`;
}

export function formatCount(n: number): string {
  return n.toLocaleString("en-US");
}

export function runsLabel(n: number): string {
  return n === 1 ? "1 run" : `${formatCount(n)} runs`;
}

export function tokensLabel(n: number): string {
  return `${formatCount(n)} tokens`;
}

export interface MergedBuckets {
  readonly buckets: readonly UsageBucket[];
  readonly bucketMs: number;
}

export function mergeBuckets(buckets: readonly UsageBucket[], bucketMs: number, target: number): MergedBuckets {
  if (buckets.length <= target) return { buckets, bucketMs };
  const size = Math.ceil(buckets.length / target);
  const merged: UsageBucket[] = [];
  for (let from = 0; from < buckets.length; from += size) {
    const group = buckets.slice(from, from + size);
    merged.push({
      start: group[0]!.start,
      runs: group.reduce((n, b) => n + b.runs, 0),
      tokens: group.reduce((n, b) => n + b.tokens, 0),
    });
  }
  return { buckets: merged, bucketMs: bucketMs * size };
}

export interface AxisTick {
  readonly at: number;
  readonly label: string;
}

export function axisTicks(buckets: readonly UsageBucket[], range: UsageRange, count: number): readonly AxisTick[] {
  if (buckets.length === 0) return [];
  const shown = Math.min(count, buckets.length);
  const last = buckets.length - 1;
  return Array.from({ length: shown }, (_, i) => {
    const at = shown === 1 ? 0 : Math.round((i * last) / (shown - 1));
    const start = new Date(buckets[at]!.start);
    return { at, label: range === "24h" ? clockOf(start) : `${pad(start.getUTCMonth() + 1)}-${pad(start.getUTCDate())}` };
  });
}

export function barPercents(values: readonly number[]): readonly number[] {
  const peak = Math.max(0, ...values);
  if (peak === 0) return values.map(() => 0);
  return values.map((value) => (value === 0 ? 0 : Math.max(MIN_VISIBLE_PERCENT, (value / peak) * 100)));
}
