import type { ConversationDetail, RepoRule, ThreadEntry } from "../../../api/types";

export interface RepoLine {
  readonly label: string;
  readonly reason: string;
}

type RepoEntry = Extract<ThreadEntry, { kind: "repo" }>;

export const RULE_REASONS: Readonly<Record<RepoRule, string>> = {
  named: "you named it",
  code: "found in its code",
  only: "the only repo",
  picked: "you picked it",
  linked: "set by hand",
};

const sameRepo = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

function newestRepoEntry(entries: readonly ThreadEntry[]): RepoEntry | null {
  return entries.reduce<RepoEntry | null>(
    (newest, entry) => (entry.kind === "repo" && (newest === null || entry.id > newest.id) ? entry : newest),
    null,
  );
}

function label(repo: string, baseBranch: string | null): string {
  return baseBranch ? `~ ${repo} · ${baseBranch}` : `~ ${repo}`;
}

function linkedLine(detail: ConversationDetail, linked: string): RepoLine {
  const task = detail.tasks.find((t) => sameRepo(t.repo, linked));
  return { label: label(linked, task?.baseBranch ?? null), reason: RULE_REASONS.linked };
}

export function repoLine(detail: ConversationDetail): RepoLine | null {
  const entry = newestRepoEntry(detail.entries);
  const linked = detail.conversation.linkedRepo;
  if (linked && (entry === null || !sameRepo(entry.repo, linked))) return linkedLine(detail, linked);
  if (entry === null) return null;
  return {
    label: label(entry.repo, entry.baseBranch),
    reason: entry.reason.trim() || RULE_REASONS[entry.rule],
  };
}
