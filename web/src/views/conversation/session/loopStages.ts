import type { Task, TaskState } from "../../../api/types";

export type LoopStage = "plan" | "code" | "test" | "review" | "ship";

export const LOOP_STAGES: readonly LoopStage[] = ["plan", "code", "test", "review", "ship"];

export const STAGE_OF: Readonly<Record<TaskState, LoopStage | null>> = {
  queued: "plan",
  working: "code",
  needsYou: "code",
  error: "code",
  inCi: "test",
  inReview: "review",
  merged: "ship",
  ended: null,
};

export interface StageCount {
  readonly stage: LoopStage;
  readonly count: number;
}

export interface LoopCounts {
  readonly stages: readonly StageCount[];
  readonly needsYou: number;
}

export function loopCounts(tasks: readonly Task[]): LoopCounts {
  return {
    stages: LOOP_STAGES.map((stage) => ({
      stage,
      count: tasks.filter((task) => STAGE_OF[task.state] === stage).length,
    })),
    needsYou: tasks.filter((task) => task.state === "needsYou").length,
  };
}
