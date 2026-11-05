import type { ClaudeUsage, UsageWindow } from "../api/types";

const HOUR_MS = 3_600_000;
const WEEKDAYS = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"] as const;

export type UsageLevel = "ok" | "warn" | "hot";

export type RunOut = { readonly kind: "reached" } | { readonly kind: "lasts" } | { readonly kind: "runsOut"; readonly at: number };

export interface WindowView {
  readonly name: string;
  readonly used: string;
  readonly reset: string;
  readonly runOut: string | null;
  readonly runsOut: boolean;
}

const two = (n: number) => String(n).padStart(2, "0");

const clock = (ms: number) => {
  const date = new Date(ms);
  return `${two(date.getHours())}:${two(date.getMinutes())}`;
};

export function resetLabel(resetsAt: number | null, now: number): string {
  if (resetsAt === null) return "";
  if (resetsAt <= now) return "now";
  const minutes = Math.floor((resetsAt - now) / 60_000);
  if (minutes < 1) return "<1m";
  const hours = Math.floor(minutes / 60);
  if (hours >= 24) return `${Math.floor(hours / 24)}d ${hours % 24}h`;
  return hours ? `${hours}h ${minutes % 60}m` : `${minutes}m`;
}

export function resetTimeLabel(resetsAt: number | null, now: number): string {
  if (resetsAt === null) return "";
  const sameDay = new Date(resetsAt).toDateString() === new Date(now).toDateString();
  return sameDay ? clock(resetsAt) : `${WEEKDAYS[new Date(resetsAt).getDay()]} ${clock(resetsAt)}`;
}

export function resetSentence(resetsAt: number | null, now: number): string {
  if (resetsAt === null) return "";
  if (resetsAt <= now) return "reset due";
  return `resets ${resetTimeLabel(resetsAt, now)} (in ${resetLabel(resetsAt, now)})`;
}

export const usageLevel = (usedPercent: number): UsageLevel =>
  usedPercent >= 80 ? "hot" : usedPercent >= 60 ? "warn" : "ok";

const resetPassed = (window: UsageWindow, now: number) => window.resetsAt !== null && window.resetsAt <= now;

export const displayWindow = (window: UsageWindow, now: number): UsageWindow =>
  resetPassed(window, now) ? { usedPercent: 0, resetsAt: null, ratePerHour: null } : window;

export function projectRunOut(window: UsageWindow, readAt: number): RunOut {
  if (window.usedPercent >= 100) return { kind: "reached" };
  if (!window.ratePerHour || window.ratePerHour <= 0 || window.resetsAt === null) return { kind: "lasts" };
  const at = readAt + ((100 - window.usedPercent) / window.ratePerHour) * HOUR_MS;
  return at >= window.resetsAt ? { kind: "lasts" } : { kind: "runsOut", at };
}

export function runOutSentence(run: RunOut, now: number): string {
  if (run.kind === "reached") return "limit reached";
  if (run.kind === "lasts") return "lasts until reset";
  const when = `runs out ~${resetTimeLabel(run.at, now)}`;
  return run.at <= now ? when : `${when} (in ${resetLabel(run.at, now)})`;
}

const percent = (window: UsageWindow) => Math.round(window.usedPercent);

export function windowView(name: string, window: UsageWindow, readAt: number, now: number): WindowView {
  if (resetPassed(window, now)) {
    return { name, used: "0% used", reset: "reset since last update", runOut: null, runsOut: false };
  }
  const run = projectRunOut(window, readAt);
  return {
    name,
    used: `${percent(window)}% used`,
    reset: resetSentence(window.resetsAt, now) || "reset time unknown",
    runOut: runOutSentence(run, now),
    runsOut: run.kind === "runsOut",
  };
}

export function meterLabel(name: string, window: UsageWindow, now: number): string {
  const shown = displayWindow(window, now);
  const reset = resetSentence(shown.resetsAt, now);
  const used = `${name} ${percent(shown)}% used`;
  return reset ? `${used} · ${reset}` : used;
}

export const isStale = (usage: ClaudeUsage | null): boolean =>
  usage !== null && usage.error !== null && (usage.session !== null || usage.weekly !== null);

export function footLabel(usage: ClaudeUsage): string {
  const updated = `Updated ${clock(usage.updatedAt)}`;
  return usage.error === null ? updated : `Stale · ${updated}`;
}

export function staleReason(error: string | null): string {
  if (error === null) return "";
  if (error.startsWith("Sign in")) return "Sign in to Claude Code to refresh";
  if (error.includes("rate limited")) return "Rate limited, retrying later";
  if (error.includes("offline")) return "Offline";
  return error;
}
