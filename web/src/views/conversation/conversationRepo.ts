import type { ConversationDetail, Repo, Task, ThreadEntry } from "../../api/types";

export type ConversationRepo =
  | { readonly kind: "repo"; readonly name: string; readonly branch: string | null }
  | { readonly kind: "multiple" }
  | { readonly kind: "none" };

const NONE: ConversationRepo = { kind: "none" };
const MULTIPLE: ConversationRepo = { kind: "multiple" };

function sameName(a: string, b: string): boolean {
  return a.toLowerCase() === b.toLowerCase();
}

function linked(name: string, repos: readonly Repo[] | null): ConversationRepo {
  const known = repos?.find((repo) => sameName(repo.name, name));
  return { kind: "repo", name, branch: known?.baseBranch ?? null };
}

function fromTasks(tasks: readonly Task[]): ConversationRepo | null {
  const counts = tasks.reduce<ReadonlyMap<string, number>>(
    (acc, task) => new Map(acc).set(task.repo, (acc.get(task.repo) ?? 0) + 1),
    new Map(),
  );
  const ranked = [...counts.entries()].sort((a, b) => b[1] - a[1]);
  const [top, second] = ranked;
  if (top === undefined) return null;
  if (second !== undefined && second[1] === top[1]) return MULTIPLE;
  const first = tasks.find((task) => task.repo === top[0]);
  return { kind: "repo", name: top[0], branch: first?.baseBranch ?? null };
}

function fromEntries(entries: readonly ThreadEntry[]): ConversationRepo | null {
  const latest = [...entries].reverse().find((entry) => entry.kind === "repo");
  return latest?.kind === "repo" ? { kind: "repo", name: latest.repo, branch: latest.baseBranch } : null;
}

export function conversationRepo(detail: ConversationDetail, repos: readonly Repo[] | null): ConversationRepo {
  const hand = detail.conversation.linkedRepo;
  if (hand !== null && hand !== "") return linked(hand, repos);
  return fromTasks(detail.tasks) ?? fromEntries(detail.entries) ?? NONE;
}

export function repoText(repo: ConversationRepo): string {
  switch (repo.kind) {
    case "repo":
      return repo.branch === null ? `~ ${repo.name}` : `~ ${repo.name} · ${repo.branch}`;
    case "multiple":
      return "multiple repos";
    case "none":
      return "no repo";
  }
}
