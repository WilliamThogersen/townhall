import type { Task, TaskState } from "../../../api/types";
import { taskView } from "../session/taskRows";

export type TaskGlyphState = "queued" | "working" | "waiting" | "ci" | "merged" | "dead" | "error";

const GLYPH: Readonly<Record<TaskState, TaskGlyphState>> = {
  queued: "queued",
  working: "working",
  needsYou: "waiting",
  inCi: "ci",
  inReview: "ci",
  merged: "merged",
  ended: "dead",
  error: "error",
};

const FINISHED: ReadonlySet<TaskState> = new Set(["merged", "ended", "error"]);

export function taskGlyph(state: TaskState): TaskGlyphState {
  return GLYPH[state];
}

export function taskWords(task: Task, tasks: readonly Task[]): string {
  return taskView(task, tasks).words;
}

export function unfinishedCount(tasks: readonly Task[]): number {
  return tasks.filter((task) => !FINISHED.has(task.state)).length;
}

export function needsYouCount(tasks: readonly Task[]): number {
  return tasks.filter((task) => task.state === "needsYou").length;
}

function plural(count: number, word: string): string {
  return `${count} ${word}${count === 1 ? "" : "s"}`;
}

export function taskBlockHead(tasks: readonly Task[], count: number): string {
  const repos = [...new Set(tasks.map((task) => task.repo))];
  const head = plural(count, "task");
  const [only] = repos;
  if (repos.length === 1 && only !== undefined) {
    const branch = tasks[0]?.baseBranch;
    return branch ? `${head} for ${only} · ${branch}` : `${head} for ${only}`;
  }
  return repos.length > 1 ? `${head} for ${repos.join(", ")}` : head;
}
