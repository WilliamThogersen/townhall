import type { Conversation, EntryBody, RepoRule, Task, ThreadEntry } from "../../../api/types";

export type StatusWords = "thinking" | "your turn" | "ended";

const RULE_REASON: Readonly<Record<RepoRule, string>> = {
  named: "you named it",
  code: "found in its code",
  only: "only repo",
  picked: "you picked it",
  linked: "set by hand",
};

type RepoBody = Extract<EntryBody, { kind: "repo" }>;

function pad(n: number): string {
  return String(n).padStart(2, "0");
}

export function clockTime(iso: string): string {
  const at = new Date(iso);
  if (Number.isNaN(at.getTime())) return "";
  return `${pad(at.getHours())}:${pad(at.getMinutes())}`;
}

export function repoReason(entry: RepoBody): string {
  return entry.reason.trim() === "" ? RULE_REASON[entry.rule] : entry.reason;
}

export function isNeedsYouOpen(entries: readonly ThreadEntry[], index: number, tasks: readonly Task[]): boolean {
  const entry = entries[index];
  if (entry?.kind !== "needsYou") return false;
  const task = tasks.find((t) => t.name === entry.agent);
  if (task !== undefined && task.state !== "needsYou") return false;
  return !entries
    .slice(index + 1)
    .some(
      (later) =>
        (later.kind === "agent" && later.agent === entry.agent && later.event === "sent") ||
        (later.kind === "needsYou" && later.agent === entry.agent),
    );
}

export function isRepoAskOpen(entries: readonly ThreadEntry[], index: number): boolean {
  return !entries.slice(index + 1).some((later) => later.kind === "you");
}

export interface AskChoices {
  readonly candidates: readonly string[];
  readonly all: "both" | "all" | null;
}

export function askChoices(candidates: readonly string[], offerAll: boolean): AskChoices {
  if (!offerAll) return { candidates, all: null };
  return { candidates, all: candidates.length === 2 ? "both" : "all" };
}

export function conversationStatus(conversation: Conversation): StatusWords {
  if (conversation.status === "thinking") return "thinking";
  return conversation.ended ? "ended" : "your turn";
}

export function endQuestion(title: string, unfinished: number): string {
  const tasks = unfinished === 1 ? "1 task keeps running" : `${unfinished} tasks keep running`;
  return `end ${title}? ${tasks}`;
}
