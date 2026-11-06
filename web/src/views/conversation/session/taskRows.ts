import type { Task, TaskState } from "../../../api/types";
import type { AgentState } from "../../../ui/StatusGlyph";

export type TaskGlyphState = AgentState | "queued" | "ci" | "merged";

export type TaskTone = "waiting" | "error" | "merged" | "dead";

export interface TaskView {
  readonly task: Task;
  readonly glyph: TaskGlyphState;
  readonly tone: TaskTone | null;
  readonly words: string;
  readonly blocked: string | null;
  readonly prOpen: boolean;
}

interface StateLook {
  readonly glyph: TaskGlyphState;
  readonly tone: TaskTone | null;
  readonly words: string;
  readonly rank: number;
}

const LOOKS: Readonly<Record<TaskState, StateLook>> = {
  needsYou: { glyph: "waiting", tone: "waiting", words: "needs you", rank: 0 },
  error: { glyph: "error", tone: "error", words: "failed", rank: 1 },
  working: { glyph: "working", tone: null, words: "working", rank: 2 },
  queued: { glyph: "queued", tone: null, words: "queued", rank: 3 },
  inCi: { glyph: "ci", tone: null, words: "in ci", rank: 4 },
  inReview: { glyph: "ci", tone: null, words: "in review", rank: 5 },
  merged: { glyph: "merged", tone: "merged", words: "merged", rank: 6 },
  ended: { glyph: "dead", tone: "dead", words: "ended", rank: 7 },
};

const SUMMARY_ORDER: readonly TaskState[] = [
  "needsYou",
  "error",
  "working",
  "queued",
  "inCi",
  "inReview",
  "merged",
  "ended",
];

const FAILED_CHECKS: StateLook = { glyph: "error", tone: "error", words: "checks failed", rank: LOOKS.error.rank };

const FINISHED: ReadonlySet<TaskState> = new Set(["ended", "merged"]);

const sameName = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

const CHECKED_STATES: ReadonlySet<TaskState> = new Set(["inCi", "error"]);

function lookOf(task: Task): StateLook {
  const checks = task.pr?.checks;
  if (checks === "failed" && CHECKED_STATES.has(task.state)) return FAILED_CHECKS;
  if (checks === "passed" && task.state === "inCi") return { ...LOOKS.inCi, words: "checks passed" };
  return LOOKS[task.state];
}

export function pendingAfter(task: Task, siblings: readonly Task[]): readonly string[] {
  return task.after.filter((name) => {
    const found = siblings.find((other) => sameName(other.name, name));
    return found === undefined || !FINISHED.has(found.state);
  });
}

function blockedOf(task: Task, siblings: readonly Task[]): string | null {
  if (task.state !== "queued" || pendingAfter(task, siblings).length > 0) return null;
  return task.blocked;
}

function wordsOf(task: Task, look: StateLook, siblings: readonly Task[], blocked: string | null): string {
  if (task.state !== "queued") return look.words;
  const waiting = pendingAfter(task, siblings);
  if (waiting.length > 0) return `after ${waiting.join(", ")}`;
  return blocked === null ? look.words : "blocked";
}

export function taskView(task: Task, siblings: readonly Task[]): TaskView {
  const look = lookOf(task);
  const blocked = blockedOf(task, siblings);
  return {
    task,
    glyph: look.glyph,
    tone: look.tone,
    words: wordsOf(task, look, siblings, blocked),
    blocked,
    prOpen: task.pr !== null && !FINISHED.has(task.state),
  };
}

export function sortTasks(tasks: readonly Task[]): readonly Task[] {
  return [...tasks].sort((a, b) => lookOf(a).rank - lookOf(b).rank);
}

export function taskViews(tasks: readonly Task[]): readonly TaskView[] {
  return sortTasks(tasks).map((task) => taskView(task, tasks));
}

export function taskCount(n: number): string {
  return `${n} ${n === 1 ? "task" : "tasks"}`;
}

export function taskSummary(tasks: readonly Task[]): string {
  const parts = SUMMARY_ORDER.flatMap((state) => {
    const count = tasks.filter((task) => task.state === state).length;
    return count > 0 ? [`${count} ${LOOKS[state].words}`] : [];
  });
  return [taskCount(tasks.length), ...parts].join(" · ");
}

export function sameTask(a: string | null, b: string): boolean {
  return a !== null && sameName(a, b);
}
