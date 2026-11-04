import { list, listOf, nullable, num, oneOf, record, str, type Guard } from "./guardKit";
import type { ShippedDay, Stats, Step, StepResult } from "./types";

const STEP_RESULTS = ["running", "ok", "error"] as const satisfies readonly StepResult[];

export function parseStep(value: unknown): Step {
  const r = record(value, "step");
  return {
    id: str(r.id, "step.id"),
    tool: str(r.tool, "step.tool"),
    summary: str(r.summary, "step.summary"),
    startedAt: str(r.startedAt, "step.startedAt"),
    endedAt: nullable(r.endedAt, "step.endedAt", str),
    result: oneOf(STEP_RESULTS)(r.result, "step.result"),
  };
}

export const parseSteps: Guard<readonly Step[]> = listOf(parseStep, "steps");

export function parseShippedDay(value: unknown): ShippedDay {
  const r = record(value, "day");
  return {
    date: str(r.date, "day.date"),
    fixes: num(r.fixes, "day.fixes"),
    features: num(r.features, "day.features"),
    lines: num(r.lines, "day.lines"),
    agents: num(r.agents, "day.agents"),
  };
}

export function parseStats(value: unknown): Stats {
  const r = record(value, "stats");
  return { days: list(r.days, "stats.days", parseShippedDay) };
}
