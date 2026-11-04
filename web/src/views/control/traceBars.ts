import type { AgentInfo, Step, Task, TaskState } from "../../api/types";
import { stageOf, stepKind } from "./stage";
import { durationLabel, spanPercents, type TimeAxis } from "./timeAxis";
import { conversationEnd, type ScopedConversation } from "./traceScope";

export type GlyphState = "alive" | "working" | "waiting" | "error" | "dead" | "queued" | "ci" | "merged";
export type BarState = "running" | "waiting" | "queued" | "error";
export type TagTone = "waiting" | "error" | "ok";

export interface Bar {
  readonly start: number;
  readonly len: number;
  readonly state: BarState | null;
  readonly label: string;
}

export interface Tag {
  readonly text: string;
  readonly tone: TagTone | null;
}

export const SLOWEST_AFTER = 3;

const TASK_GLYPHS: Readonly<Record<TaskState, GlyphState>> = {
  queued: "queued",
  working: "working",
  needsYou: "waiting",
  inCi: "ci",
  inReview: "alive",
  merged: "merged",
  ended: "dead",
  error: "error",
};

const TASK_BARS: Readonly<Partial<Record<TaskState, BarState>>> = {
  working: "running",
  needsYou: "waiting",
  inCi: "running",
  error: "error",
};

const RUNS_TO_NOW: ReadonlySet<TaskState> = new Set(["working", "needsYou", "inCi"]);

const joined = (...parts: (string | null)[]) => parts.filter((part) => part).join(" · ");

function bar(axis: TimeAxis, from: number, to: number, state: BarState | null, label: string): Bar {
  return { ...spanPercents(axis, from, to), state, label };
}

export function conversationBar(entry: ScopedConversation, axis: TimeAxis): Bar {
  const { conversation } = entry;
  const from = Date.parse(conversation.createdAt);
  const to = conversationEnd(entry, axis.now);
  const thinking = conversation.status === "thinking";
  const status = thinking ? "thinking" : conversation.ended ? "ended" : "your turn";
  return bar(axis, from, to, thinking ? "running" : null, joined(durationLabel(to - from), status));
}

export function taskGlyph(task: Task): GlyphState {
  return TASK_GLYPHS[task.state];
}

export function taskMeta(task: Task, latest: Step | null): string {
  if (task.state === "queued" || task.state === "ended") return task.state;
  return stageOf(task, latest) ?? task.state;
}

export function taskTag(task: Task): Tag | null {
  if (task.state === "needsYou") return { text: "needs you", tone: "waiting" };
  if (task.state === "inReview" && task.pr?.checks === "passed") return { text: "checks passed", tone: "ok" };
  return null;
}

const prSuffix = (task: Task, words: string) => (task.pr ? `${words} #${task.pr.number}` : words);

function errorWords(task: Task, agent: AgentInfo | null): string {
  if (task.pr?.checks === "failed") return "checks failed";
  const exitCode = agent?.state.kind === "error" ? agent.state.exitCode : null;
  return exitCode === null ? "failed" : `exited ${exitCode}`;
}

function outcome(task: Task, agent: AgentInfo | null): string | null {
  switch (task.state) {
    case "inCi":
      return prSuffix(task, "in ci");
    case "inReview":
      return prSuffix(task, "in review");
    case "merged":
      return prSuffix(task, "merged");
    case "ended":
      return "ended";
    case "error":
      return errorWords(task, agent);
    default:
      return null;
  }
}

function lastEnd(steps: readonly Step[]): number | null {
  const last = steps.at(-1);
  return last ? Date.parse(last.endedAt ?? last.startedAt) : null;
}

export function taskBar(task: Task, steps: readonly Step[], agent: AgentInfo | null, axis: TimeAxis): Bar {
  if (task.state === "queued") {
    const waitsFor = task.after.length > 0 ? `after ${task.after.join(", ")}` : "queued";
    return bar(axis, axis.now, axis.now, "queued", waitsFor);
  }
  const from = Date.parse(task.createdAt);
  const to = RUNS_TO_NOW.has(task.state) ? axis.now : lastEnd(steps);
  const state = TASK_BARS[task.state] ?? null;
  if (to === null) return bar(axis, from, from, state, outcome(task, agent) ?? "");
  return bar(axis, from, to, state, joined(durationLabel(to - from), outcome(task, agent)));
}

const isAsking = (step: Step) => step.result === "running" && stepKind(step) === "ask";

function stepState(step: Step): BarState | null {
  if (isAsking(step)) return "waiting";
  if (step.result === "running") return "running";
  return step.result === "error" ? "error" : null;
}

export function stepBar(step: Step, axis: TimeAxis): Bar {
  const from = Date.parse(step.startedAt);
  const to = step.endedAt === null ? axis.now : Date.parse(step.endedAt);
  const running = step.result === "running" ? "running" : null;
  return bar(axis, from, to, stepState(step), joined(durationLabel(to - from), running));
}

export function stepTag(step: Step, slowest: boolean): Tag | null {
  if (isAsking(step)) return { text: "needs you", tone: "waiting" };
  if (step.result === "error") return { text: "failed", tone: "error" };
  return slowest ? { text: "slowest", tone: null } : null;
}

export function slowestStep(steps: readonly Step[]): string | null {
  const finished = steps.filter((step) => step.endedAt !== null);
  if (finished.length < SLOWEST_AFTER) return null;
  const took = (step: Step) => Date.parse(step.endedAt ?? step.startedAt) - Date.parse(step.startedAt);
  return finished.reduce((slowest, step) => (took(step) > took(slowest) ? step : slowest)).id;
}
