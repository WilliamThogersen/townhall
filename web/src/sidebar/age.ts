const MINUTE_MS = 60_000;
const HOUR_MS = 60 * MINUTE_MS;
const DAY_MS = 24 * HOUR_MS;
const YEAR_MS = 365 * DAY_MS;

const STEPS: readonly { readonly ms: number; readonly unit: string }[] = [
  { ms: YEAR_MS, unit: "y" },
  { ms: DAY_MS, unit: "d" },
  { ms: HOUR_MS, unit: "h" },
  { ms: MINUTE_MS, unit: "m" },
];

export function ageLabel(time: string, now: number): string {
  const then = Date.parse(time);
  if (Number.isNaN(then)) return "";
  const elapsed = now - then;
  const step = STEPS.find(({ ms }) => elapsed >= ms);
  return step ? `${Math.floor(elapsed / step.ms)}${step.unit}` : "now";
}
