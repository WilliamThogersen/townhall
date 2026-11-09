import type { AgentRun } from "../../api/types";

/** Runs listed before "show more". */
export const RUNS_SHOWN = 8;

export function shownRuns(runs: readonly AgentRun[], all: boolean): readonly AgentRun[] {
  return all ? runs : runs.slice(0, RUNS_SHOWN);
}

export interface RunModel {
  readonly name: string;
  /** True when the run recorded no model and this is the agent type's own. */
  readonly inferred: boolean;
}

export function runModel(run: AgentRun, typeModel: string | null): RunModel | null {
  if (run.model !== null) return { name: run.model, inferred: false };
  return typeModel === null ? null : { name: typeModel, inferred: true };
}

export function showsModelColumn(runs: readonly AgentRun[], typeModel: string | null): boolean {
  return runs.some((run) => runModel(run, typeModel) !== null);
}
