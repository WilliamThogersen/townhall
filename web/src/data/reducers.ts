import type {
  AgentInfo,
  Conversation,
  ConversationDetail,
  Repo,
  ServerEvent,
  Task,
  ThreadEntry,
} from "../api/types";
import { loaded, mapDetails, mapResource, withDetail, type DataState } from "./state";

const byNewestActivity = (a: Conversation, b: Conversation): number =>
  b.updatedAt.localeCompare(a.updatedAt);

export function newestFirst(list: readonly Conversation[]): readonly Conversation[] {
  return [...list].sort(byNewestActivity);
}

const sameName = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();

function isStale(next: Conversation, current: Conversation | undefined): boolean {
  return current !== undefined && next.updatedAt < current.updatedAt;
}

export function upsertConversation(
  list: readonly Conversation[],
  conversation: Conversation,
): readonly Conversation[] {
  const current = list.find((c) => c.id === conversation.id);
  if (isStale(conversation, current)) return list;
  return newestFirst([conversation, ...list.filter((c) => c.id !== conversation.id)]);
}

export function upsertEntry(entries: readonly ThreadEntry[], entry: ThreadEntry): readonly ThreadEntry[] {
  const last = entries.at(-1);
  if (last === undefined || last.id < entry.id) return [...entries, entry];
  return [...entries.filter((e) => e.id !== entry.id), entry].sort((a, b) => a.id - b.id);
}

function upsertBy<T>(list: readonly T[], item: T, same: (a: T, b: T) => boolean): readonly T[] {
  return list.some((x) => same(x, item))
    ? list.map((x) => (same(x, item) ? item : x))
    : [...list, item];
}

export const upsertTask = (list: readonly Task[], task: Task): readonly Task[] =>
  upsertBy(list, task, (a, b) => sameName(a.name, b.name));

export const upsertAgent = (list: readonly AgentInfo[], agent: AgentInfo): readonly AgentInfo[] =>
  upsertBy(list, agent, (a, b) => sameName(a.repo, b.repo) && sameName(a.name, b.name));

export const upsertRepo = (list: readonly Repo[], repo: Repo): readonly Repo[] =>
  upsertBy(list, repo, (a, b) => sameName(a.name, b.name));

export const withoutRepo = (list: readonly Repo[], name: string): readonly Repo[] =>
  list.filter((repo) => !sameName(repo.name, name));

export function reduceConversations(
  list: readonly Conversation[],
  event: ServerEvent,
): readonly Conversation[] {
  return event.type === "conversationUpdated" ? upsertConversation(list, event.conversation) : list;
}

export function reduceDetail(detail: ConversationDetail, event: ServerEvent): ConversationDetail {
  const id = detail.conversation.id;
  switch (event.type) {
    case "conversationUpdated":
      return event.conversation.id !== id || isStale(event.conversation, detail.conversation)
        ? detail
        : { ...detail, conversation: event.conversation };
    case "entryAppended":
      return event.conversationId === id
        ? { ...detail, entries: upsertEntry(detail.entries, event.entry) }
        : detail;
    case "taskUpdated":
      return event.task.conversationId === id
        ? { ...detail, tasks: upsertTask(detail.tasks, event.task) }
        : detail;
    default:
      return detail;
  }
}

export function reduceTasks(list: readonly Task[], event: ServerEvent): readonly Task[] {
  return event.type === "taskUpdated" ? upsertTask(list, event.task) : list;
}

export function reduceAgents(list: readonly AgentInfo[], event: ServerEvent): readonly AgentInfo[] {
  return event.type === "agentUpdated" ? upsertAgent(list, event.agent) : list;
}

export const keepRepos = (list: readonly Repo[]): readonly Repo[] => list;

export function applyEvent(state: DataState, event: ServerEvent): DataState {
  const next: DataState = {
    ...state,
    conversations: mapResource(state.conversations, (list) => reduceConversations(list, event)),
    details: mapDetails(state.details, (detail) => reduceDetail(detail, event)),
    tasks: mapResource(state.tasks, (list) => reduceTasks(list, event)),
    agents: mapResource(state.agents, (list) => reduceAgents(list, event)),
  };
  const changed = (["conversations", "details", "tasks", "agents"] as const).some(
    (field) => next[field] !== state[field],
  );
  return changed ? next : state;
}

export function applyStarted(state: DataState, detail: ConversationDetail): DataState {
  const conversation = detail.conversation;
  return {
    ...state,
    conversations: mapResource(state.conversations, (list) => upsertConversation(list, conversation)),
    details: withDetail(state.details, conversation.id, loaded(detail)),
    tasks: mapResource(state.tasks, (list) => detail.tasks.reduce(upsertTask, list)),
  };
}
