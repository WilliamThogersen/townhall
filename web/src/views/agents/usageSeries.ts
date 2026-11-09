const MIN_VISIBLE_PERCENT = 4;

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

export function barPercents(values: readonly number[]): readonly number[] {
  const peak = Math.max(0, ...values);
  if (peak === 0) return values.map(() => 0);
  return values.map((value) => (value === 0 ? 0 : Math.max(MIN_VISIBLE_PERCENT, (value / peak) * 100)));
}
