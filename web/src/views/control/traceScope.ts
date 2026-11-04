import type { Conversation, Task, TaskState } from "../../api/types";
import { startOfDay } from "./timeAxis";

export const MULTIPLE_REPOS = "multiple repos";
export const NO_REPO = "no repo";

const LIVE_STATES: ReadonlySet<TaskState> = new Set(["queued", "working", "needsYou", "inCi", "inReview"]);

export interface ControlFilter {
  readonly conversationId: string | null;
  readonly repo: string | null;
}

export interface ScopedConversation {
  readonly conversation: Conversation;
  readonly tasks: readonly Task[];
  readonly repo: string;
  readonly live: boolean;
}

export function isLiveTask(task: Task): boolean {
  return LIVE_STATES.has(task.state);
}

export function conversationRepo(conversation: Conversation, tasks: readonly Task[]): string {
  if (conversation.linkedRepo) return conversation.linkedRepo;
  const counts = tasks.reduce<ReadonlyMap<string, number>>(
    (acc, task) => new Map(acc).set(task.repo, (acc.get(task.repo) ?? 0) + 1),
    new Map(),
  );
  const ranked = [...counts.entries()].sort((a, b) => b[1] - a[1]);
  const [first, second] = ranked;
  if (first === undefined) return NO_REPO;
  return second !== undefined && second[1] === first[1] ? MULTIPLE_REPOS : first[0];
}

function tasksByConversation(tasks: readonly Task[]): ReadonlyMap<string, readonly Task[]> {
  return tasks.reduce<ReadonlyMap<string, readonly Task[]>>(
    (acc, task) => new Map(acc).set(task.conversationId, [...(acc.get(task.conversationId) ?? []), task]),
    new Map(),
  );
}

const sameRepo = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

function scoped(conversation: Conversation, tasks: readonly Task[]): ScopedConversation {
  const live = conversation.status === "thinking" || tasks.some(isLiveTask);
  return { conversation, tasks, repo: conversationRepo(conversation, tasks), live };
}

function inRepo(entry: ScopedConversation, repo: string | null): ScopedConversation | null {
  if (repo === null) return entry;
  const tasks = entry.tasks.filter((task) => sameRepo(task.repo, repo));
  if (tasks.length === 0 && !sameRepo(entry.repo, repo)) return null;
  return { ...entry, tasks };
}

function ranToday(entry: ScopedConversation, dayStart: number): boolean {
  return entry.live || Date.parse(entry.conversation.updatedAt) >= dayStart;
}

const byCreated = (a: ScopedConversation, b: ScopedConversation) =>
  Date.parse(a.conversation.createdAt) - Date.parse(b.conversation.createdAt);

export function scopeTrace(
  conversations: readonly Conversation[],
  tasks: readonly Task[],
  filter: ControlFilter,
  now: number,
): readonly ScopedConversation[] {
  const grouped = tasksByConversation(tasks);
  const dayStart = startOfDay(now);
  const wanted = (c: Conversation) => (filter.conversationId === null ? true : c.id === filter.conversationId);
  return conversations
    .filter(wanted)
    .map((c) => scoped(c, grouped.get(c.id) ?? []))
    .filter((entry) => filter.conversationId !== null || ranToday(entry, dayStart))
    .map((entry) => inRepo(entry, filter.repo))
    .filter((entry): entry is ScopedConversation => entry !== null)
    .sort(byCreated);
}

export function conversationEnd(entry: ScopedConversation, now: number): number {
  return entry.live ? now : Date.parse(entry.conversation.updatedAt);
}

function shownStart(entry: ScopedConversation, now: number, dayStart: number): number {
  const start = Date.parse(entry.conversation.createdAt);
  if (entry.live || conversationEnd(entry, now) < dayStart) return start;
  return Math.max(start, dayStart);
}

export function earliestStart(entries: readonly ScopedConversation[], now: number): number {
  const dayStart = startOfDay(now);
  return entries.reduce((earliest, entry) => Math.min(earliest, shownStart(entry, now, dayStart)), now);
}
