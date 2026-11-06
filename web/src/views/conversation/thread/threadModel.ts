import type { Conversation, EntryBody, RepoRule, Task, ThreadEntry } from "../../../api/types";
import type { ReplyTarget } from "../../../composer/replies";

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

const EXCERPT_CHARS = 120;

function said(body: EntryBody): { readonly author: string; readonly text: string } | null {
  switch (body.kind) {
    case "you":
      return { author: "you", text: body.text };
    case "townhall":
      return { author: "townhall", text: body.text };
    case "repoAsk":
      return { author: "townhall", text: body.question };
    case "needsYou":
      return { author: body.agent, text: body.question };
    case "agent":
      return { author: body.agent, text: body.text === "" ? body.event : body.text };
    case "event":
      return { author: "task event", text: body.text };
    case "handoff":
      return { author: body.from, text: body.reason };
    case "error":
      return { author: "townhall", text: body.message };
    case "repo":
    case "tasksCreated":
      return null;
  }
}

export function excerptOf(text: string): string {
  const joined = text.split(/\s+/).filter((word) => word !== "").join(" ");
  const chars = Array.from(joined);
  return chars.length > EXCERPT_CHARS ? `${chars.slice(0, EXCERPT_CHARS).join("")}…` : joined;
}

export function replyTarget(entry: ThreadEntry): ReplyTarget | null {
  const line = said(entry);
  return line === null ? null : { id: entry.id, author: line.author, excerpt: excerptOf(line.text) };
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

/**
 * Which `sent` line answered which agent question: question index to answer index. The
 * answer is the first `sent` to the same agent before that agent asks again.
 */
export function askAnswers(entries: readonly ThreadEntry[]): ReadonlyMap<number, number> {
  const pairs = entries.flatMap((entry, i) => {
    if (entry.kind !== "needsYou") return [];
    const later = entries.slice(i + 1);
    const end = later.findIndex((e) => e.kind === "needsYou" && e.agent === entry.agent);
    const window = end === -1 ? later : later.slice(0, end);
    const answer = window.findIndex((e) => e.kind === "agent" && e.agent === entry.agent && e.event === "sent");
    return answer === -1 ? [] : [[i, i + 1 + answer] as const];
  });
  return new Map(pairs);
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
  const tasks =
    unfinished === 1
      ? "1 task stops and its worktree is deleted"
      : `${unfinished} tasks stop and their worktrees are deleted`;
  return `end ${title}? ${tasks}`;
}
