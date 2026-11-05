import type { AgentTypeSummary, AgentUsage } from "../../api/types";

export type PipelineStep = "plan" | "code" | "test" | "review" | "ship";

const STEP_OF: Readonly<Record<string, PipelineStep>> = {
  planner: "plan",
  coder: "code",
  tester: "test",
  reviewer: "review",
  shipper: "ship",
};

const FALLBACK_DEFAULT = "builder";

export function pipelineStep(name: string): PipelineStep | null {
  return STEP_OF[name.toLowerCase()] ?? null;
}

const byName = (a: AgentTypeSummary, b: AgentTypeSummary) =>
  a.name.toLowerCase().localeCompare(b.name.toLowerCase());

export function defaultTypeName(types: readonly AgentTypeSummary[]): string | null {
  const builder = types.find((t) => t.name.toLowerCase() === FALLBACK_DEFAULT);
  if (builder) return builder.name;
  return [...types].sort(byName)[0]?.name ?? null;
}

export interface Usage {
  readonly runs: number;
  readonly tokens: number;
}

const NO_USAGE: Usage = { runs: 0, tokens: 0 };

export function usageOf(rows: readonly AgentUsage[] | null, name: string): Usage | null {
  if (rows === null) return null;
  const found = rows.find((row) => row.agent.toLowerCase() === name.toLowerCase());
  return found ?? NO_USAGE;
}
