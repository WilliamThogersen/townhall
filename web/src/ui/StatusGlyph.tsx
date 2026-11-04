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

export const AGENT_STATE_WORDS: Readonly<Record<AgentState, string>> = {
  waiting: "waiting",
  error: "error",
  working: "working",
  alive: "running",
  dead: "ended",
};

export interface GlyphCountsProps {
  readonly counts: AgentStateCounts;
  readonly className?: string;
  readonly labelled?: boolean;
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

function spokenCounts(present: readonly StateCount[]): string {
  return present.map(({ state, count }) => `${count} ${AGENT_STATE_WORDS[state]}`).join(", ");
}

function LabelledCounts({ present }: { readonly present: readonly StateCount[] }) {
  return (
    <>
      {present.map(({ state, count }) => (
        <span key={state} aria-hidden="true">
          <StatusGlyph state={state} />
          {`${count} ${AGENT_STATE_WORDS[state]}`}
        </span>
      ))}
      <span className="th-sr-only">{spokenCounts(present)}</span>
    </>
  );
}

function BareCounts({ present, className }: { readonly present: readonly StateCount[]; readonly className?: string }) {
  const last = present.length - 1;
  return (
    <span className={className}>
      {present.map(({ state, count }, index) => (
        <Fragment key={state}>
          <StatusGlyph state={state} />
          <span aria-hidden="true">{index < last ? `${count} ` : `${count}`}</span>
        </Fragment>
      ))}
      <span className="th-sr-only">{spokenCounts(present)}</span>
    </span>
  );
}

export function GlyphCounts({ counts, className, labelled = false }: GlyphCountsProps) {
  const present = presentCounts(counts);
  if (present.length === 0) return null;
  return labelled ? <LabelledCounts present={present} /> : <BareCounts present={present} className={className} />;
}
