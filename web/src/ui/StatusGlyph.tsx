import { Fragment } from "react";
import { Spinner } from "./Spinner";

export type AgentState = "alive" | "working" | "waiting" | "error" | "dead";

export type AgentStateCounts = Readonly<Partial<Record<AgentState, number>>>;

export const AGENT_STATE_ORDER: readonly AgentState[] = ["waiting", "error", "working", "alive", "dead"];

export interface StatusGlyphProps {
  readonly state: AgentState;
}

export function StatusGlyph({ state }: StatusGlyphProps) {
  return (
    <span className="th-status" data-state={state} aria-hidden="true">
      {state === "working" ? <Spinner /> : null}
    </span>
  );
}

export interface GlyphCountsProps {
  readonly counts: AgentStateCounts;
  readonly className?: string;
}

interface StateCount {
  readonly state: AgentState;
  readonly count: number;
}

function presentCounts(counts: AgentStateCounts): readonly StateCount[] {
  return AGENT_STATE_ORDER.map((state) => ({ state, count: counts[state] ?? 0 })).filter(
    ({ count }) => Number.isFinite(count) && count > 0,
  );
}

export function GlyphCounts({ counts, className }: GlyphCountsProps) {
  const present = presentCounts(counts);
  if (present.length === 0) return null;
  const spoken = present.map(({ state, count }) => `${count} ${state}`).join(", ");
  return (
    <span className={className ? `th-glyphs ${className}` : "th-glyphs"}>
      <span aria-hidden="true">
        {present.map(({ state, count }, index) => (
          <Fragment key={state}>
            {index > 0 ? " " : null}
            <StatusGlyph state={state} />
            {count}
          </Fragment>
        ))}
      </span>
      <span className="th-sr-only">{spoken}</span>
    </span>
  );
}
