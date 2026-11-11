import { listOf, nullable, num, oneOf, record, str, type Guard } from "./guardKit";
import { withQuery } from "./query";
import { request } from "./request";
import type { Run, RunState, RunStep, RunStepState } from "./types";

export const RUN_STATES: readonly RunState[] = ["running", "needsYou", "finished", "failed"];
export const RUN_STEP_STATES: readonly RunStepState[] = [
  "pending",
  "starting",
  "started",
  "done",
  "failed",
  "skipped",
  "fixing",
  "redone",
];

function parseRunStep(value: unknown): RunStep {
  const r = record(value, "run step");
  return {
    id: str(r.id, "run step.id"),
    agent: str(r.agent, "run step.agent"),
    state: oneOf(RUN_STEP_STATES)(r.state, "run step.state"),
    round: num(r.round, "run step.round"),
    task: nullable(r.task, "run step.task", str),
    verdict: nullable(r.verdict, "run step.verdict", str),
    note: nullable(r.note, "run step.note", str),
  };
}

export function parseRun(value: unknown): Run {
  const r = record(value, "run");
  return {
    id: str(r.id, "run.id"),
    workflow: str(r.workflow, "run.workflow"),
    conversationId: str(r.conversationId, "run.conversationId"),
    repo: str(r.repo, "run.repo"),
    baseBranch: str(r.baseBranch, "run.baseBranch"),
    state: oneOf(RUN_STATES)(r.state, "run.state"),
    input: str(r.input, "run.input"),
    steps: listOf(parseRunStep, "run.steps")(r.steps),
  };
}

export const parseRuns: Guard<readonly Run[]> = listOf(parseRun, "runs");

export const listRuns = (conversation: string, signal?: AbortSignal): Promise<readonly Run[]> =>
  request("GET", withQuery("/runs", { conversation }), parseRuns, undefined, signal);

export const getRun = (id: string, signal?: AbortSignal): Promise<Run> =>
  request("GET", `/runs/${encodeURIComponent(id)}`, parseRun, undefined, signal);
