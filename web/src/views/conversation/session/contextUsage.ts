import type { ContextUsage } from "../../../api/types";

export const HIGH_CONTEXT_PERCENT = 80;

const THOUSAND = 1_000;
const MILLION = 1_000_000;

export interface ContextView {
  readonly percent: number;
  readonly high: boolean;
  readonly words: string;
}

export function formatTokens(n: number): string {
  const thousands = Math.round(n / THOUSAND);
  if (n < THOUSAND) return String(Math.max(0, Math.round(n)));
  if (thousands < THOUSAND) return `${thousands}k`;
  return `${Math.round(n / (MILLION / 10)) / 10}m`;
}

function exactPercent({ used, limit }: ContextUsage): number {
  if (limit <= 0) return 0;
  return Math.min(100, Math.max(0, (used / limit) * 100));
}

export function contextView(usage: ContextUsage | null): ContextView | null {
  if (usage === null) return null;
  const exact = exactPercent(usage);
  return {
    percent: Math.floor(exact),
    high: exact >= HIGH_CONTEXT_PERCENT,
    words: `${formatTokens(usage.used)} of ${formatTokens(usage.limit)} tokens`,
  };
}
