import type { Step, Task, TaskState } from "../../api/types";

export type LoopStage = "plan" | "code" | "test" | "review" | "ship";

export const LOOP_STAGES: readonly LoopStage[] = ["plan", "code", "test", "review", "ship"];

export type StepKind = "read" | "search" | "edit" | "test" | "commit" | "ask" | "run" | "other";

const TOOL_KINDS: Readonly<Record<string, StepKind>> = {
  Read: "read",
  NotebookRead: "read",
  Grep: "search",
  Glob: "search",
  LS: "search",
  WebSearch: "search",
  WebFetch: "search",
  Edit: "edit",
  MultiEdit: "edit",
  Write: "edit",
  NotebookEdit: "edit",
  AskUserQuestion: "ask",
};

const COMMIT_COMMAND = /\bgit\s+commit\b/;
const TEST_COMMAND =
  /\b(?:(?:npm|pnpm|yarn|bun)\s+(?:run\s+)?test|vitest|jest|pytest|phpunit|rspec|mocha|playwright\s+test|(?:cargo|go|dotnet|mvn|make)\s+test)\b/;

const KIND_VERBS: Readonly<Partial<Record<StepKind, string>>> = {
  read: "read",
  search: "search",
  edit: "edit",
  test: "run tests",
  commit: "commit",
  ask: "ask",
  run: "run",
};

const STATE_STAGES: Readonly<Partial<Record<TaskState, LoopStage>>> = {
  queued: "plan",
  needsYou: "review",
  inCi: "review",
  inReview: "review",
  merged: "ship",
};

const FINISHED: ReadonlySet<TaskState> = new Set(["merged", "ended", "error"]);

function commandKind(command: string): StepKind {
  if (COMMIT_COMMAND.test(command)) return "commit";
  return TEST_COMMAND.test(command) ? "test" : "run";
}

export function stepKind(step: Step): StepKind {
  if (step.tool === "Bash") return commandKind(step.summary);
  return TOOL_KINDS[step.tool] ?? "other";
}

export function stepVerb(step: Step): string {
  return KIND_VERBS[stepKind(step)] ?? step.tool.toLowerCase();
}

function stageOfStep(step: Step | null): LoopStage {
  const kind = step === null ? null : stepKind(step);
  if (kind === "test") return "test";
  return kind === "ask" ? "review" : "code";
}

export function stageOf(task: Task, latest: Step | null): LoopStage | null {
  if (task.state === "ended") return null;
  if (task.state === "error" && task.pr?.checks === "failed") return "review";
  return STATE_STAGES[task.state] ?? stageOfStep(latest);
}

export function isFinished(task: Task): boolean {
  return FINISHED.has(task.state);
}

export interface StageCount {
  readonly stage: LoopStage;
  readonly count: number;
  readonly active: boolean;
}

export interface LoopSummary {
  readonly stages: readonly StageCount[];
  readonly needsYou: number;
}

export function loopSummary(tasks: readonly Task[], latestStep: (name: string) => Step | null): LoopSummary {
  const staged = tasks.map((task) => ({ task, stage: stageOf(task, latestStep(task.name)) }));
  const stages = LOOP_STAGES.map((stage) => {
    const here = staged.filter((entry) => entry.stage === stage);
    return { stage, count: here.length, active: here.some((entry) => !isFinished(entry.task)) };
  });
  return { stages, needsYou: tasks.filter((task) => task.state === "needsYou").length };
}
