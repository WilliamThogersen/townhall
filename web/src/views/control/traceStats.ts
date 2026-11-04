import type { Task } from "../../api/types";
import { startOfDay, windowLabel, type TimeAxis } from "./timeAxis";
import { stepsOf, type StepsBook } from "./traceRows";
import type { ScopedConversation } from "./traceScope";

export interface ControlStat {
  readonly label: string;
  readonly value: number;
}

const RUNNING: ReadonlySet<Task["state"]> = new Set(["working", "needsYou"]);

const isFailing = (task: Task) => task.state === "error" || (task.state !== "merged" && task.pr?.checks === "failed");

function finishedAt(task: Task, book: StepsBook): number {
  const last = stepsOf(book, task.name).at(-1);
  return Date.parse(last?.endedAt ?? last?.startedAt ?? task.createdAt);
}

export function traceStats(entries: readonly ScopedConversation[], book: StepsBook, now: number): readonly ControlStat[] {
  const tasks = entries.flatMap((entry) => entry.tasks);
  const dayStart = startOfDay(now);
  const mergedToday = tasks.filter((task) => task.state === "merged" && finishedAt(task, book) >= dayStart);
  return [
    { label: "conversations", value: entries.length },
    { label: "tasks", value: tasks.length },
    { label: "agents running", value: tasks.filter((task) => RUNNING.has(task.state)).length },
    { label: "failing", value: tasks.filter(isFailing).length },
    { label: "merged today", value: mergedToday.length },
    { label: "tool calls", value: tasks.reduce((sum, task) => sum + stepsOf(book, task.name).length, 0) },
  ];
}

const counted = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

function repoCount(entries: readonly ScopedConversation[]): number {
  const names = entries.flatMap((entry) => [
    ...(entry.conversation.linkedRepo ? [entry.conversation.linkedRepo] : []),
    ...entry.tasks.map((task) => task.repo),
  ]);
  return new Set(names.map((name) => name.toLowerCase())).size;
}

export function headCaption(
  entries: readonly ScopedConversation[],
  conversationId: string | null,
  axis: TimeAxis,
): string {
  if (conversationId !== null) {
    const title = entries[0]?.conversation.title;
    return [title, windowLabel(axis)].filter((part) => part).join(" · ");
  }
  const conversations = counted(entries.length, "conversation", "conversations");
  return [conversations, counted(repoCount(entries), "repo", "repos"), windowLabel(axis)].join(" · ");
}
