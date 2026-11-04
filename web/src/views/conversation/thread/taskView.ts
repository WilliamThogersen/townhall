import type { Task, TaskState } from "../../../api/types";

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

const WORDS: Readonly<Record<TaskState, string>> = {
  queued: "queued",
  working: "working",
  needsYou: "needs you",
  inCi: "in ci",
  inReview: "in review",
  merged: "merged",
  ended: "ended",
  error: "failed",
};

const FINISHED: ReadonlySet<TaskState> = new Set(["merged", "ended", "error"]);
const DONE_BEFORE: ReadonlySet<TaskState> = new Set(["merged", "ended"]);

export function taskGlyph(state: TaskState): TaskGlyphState {
  return GLYPH[state];
}

function waitingFor(task: Task, tasks: readonly Task[]): readonly string[] {
  return task.after.filter((name) => {
    const before = tasks.find((other) => other.name === name);
    return before === undefined || !DONE_BEFORE.has(before.state);
  });
}

export function taskWords(task: Task, tasks: readonly Task[]): string {
  if (task.pr?.checks === "failed" && (task.state === "error" || task.state === "inCi")) {
    return "checks failed";
  }
  if (task.state === "queued") {
    const open = waitingFor(task, tasks);
    if (open.length > 0) return `after ${open.join(", ")}`;
  }
  return WORDS[task.state];
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
