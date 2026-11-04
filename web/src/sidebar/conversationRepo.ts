import type { Conversation, Task } from "../api/types";

export type ConversationRepo =
  | { readonly kind: "repo"; readonly name: string }
  | { readonly kind: "multiple" }
  | { readonly kind: "none" };

export interface RepoGroup<T> {
  readonly key: string;
  readonly label: string;
  readonly items: readonly T[];
}

export const MULTIPLE_REPOS = "multiple repos";
export const NO_REPO = "no repo";

const MULTIPLE_KEY = "multiple";
const NONE_KEY = "none";

export function inferRepo(tasks: readonly Task[]): ConversationRepo {
  const counts = tasks.reduce((all, task) => {
    const key = task.repo.toLowerCase();
    const seen = all.get(key);
    return new Map(all).set(key, { name: seen?.name ?? task.repo, count: (seen?.count ?? 0) + 1 });
  }, new Map<string, { readonly name: string; readonly count: number }>());
  const [top, next] = [...counts.values()].sort((a, b) => b.count - a.count);
  if (!top) return { kind: "none" };
  if (next && next.count === top.count) return { kind: "multiple" };
  return { kind: "repo", name: top.name };
}

export function repoOfConversation(conversation: Conversation, tasks: readonly Task[]): ConversationRepo {
  if (conversation.linkedRepo !== null) return { kind: "repo", name: conversation.linkedRepo };
  return inferRepo(tasks);
}

export function repoLabel(repo: ConversationRepo): string {
  if (repo.kind === "repo") return repo.name;
  return repo.kind === "multiple" ? MULTIPLE_REPOS : NO_REPO;
}

export const rowRepoText = (repo: ConversationRepo): string => (repo.kind === "none" ? "" : repoLabel(repo));

function groupKey(repo: ConversationRepo): string {
  if (repo.kind === "repo") return `repo:${repo.name.toLowerCase()}`;
  return repo.kind === "multiple" ? MULTIPLE_KEY : NONE_KEY;
}

const rank = (key: string) => (key === MULTIPLE_KEY ? 1 : key === NONE_KEY ? 2 : 0);

export function groupByRepo<T>(items: readonly T[], repoOf: (item: T) => ConversationRepo): readonly RepoGroup<T>[] {
  const groups = items.reduce((all, item) => {
    const repo = repoOf(item);
    const key = groupKey(repo);
    const group = all.get(key) ?? { key, label: repoLabel(repo), items: [] };
    return new Map(all).set(key, { ...group, items: [...group.items, item] });
  }, new Map<string, RepoGroup<T>>());
  return [...groups.values()].sort((a, b) => rank(a.key) - rank(b.key));
}
