import { nullable, num, record, str } from "./guardKit";
import type { ClaudeUsage, UsageWindow } from "./types";

function parseWindow(value: unknown, what: string): UsageWindow {
  const r = record(value, what);
  return {
    usedPercent: num(r.usedPercent, `${what}.usedPercent`),
    resetsAt: nullable(r.resetsAt, `${what}.resetsAt`, num),
    ratePerHour: nullable(r.ratePerHour, `${what}.ratePerHour`, num),
  };
}

export function parseUsage(value: unknown): ClaudeUsage {
  const r = record(value, "usage");
  return {
    session: nullable(r.session, "usage.session", parseWindow),
    weekly: nullable(r.weekly, "usage.weekly", parseWindow),
    updatedAt: num(r.updatedAt, "usage.updatedAt"),
    error: nullable(r.error, "usage.error", str),
  };
}
