import { describe, expect, it } from "vitest";
import { aConversation, anEntry, aTask, aYouEntry } from "../../../api/fixtures";
import type { EntryBody } from "../../../api/types";
import {
  askAnswers,
  askChoices,
  clockTime,
  conversationStatus,
  endQuestion,
  isNeedsYouOpen,
  isRepoAskOpen,
  repoReason,
  replyTarget,
} from "./threadModel";

const ask: EntryBody = { kind: "needsYou", agent: "fix-login", question: "commit? (y/n)", choices: ["y", "n"] };

describe("clockTime", () => {
  it("shows the local hour and minute", () => {
    expect(clockTime(new Date(2026, 10, 4, 9, 5).toISOString())).toBe("09:05");
    expect(clockTime(new Date(2026, 10, 4, 23, 59).toISOString())).toBe("23:59");
  });

  it("shows nothing for a broken time", () => {
    expect(clockTime("not a time")).toBe("");
  });
});

describe("replyTarget", () => {
  const target = (body: EntryBody) => replyTarget(anEntry(7, body));

  it("names who said it and what", () => {
    expect(target({ kind: "you", text: "use the new login", attachments: [] })).toEqual({
      id: 7,
      author: "you",
      excerpt: "use the new login",
    });
    expect(target({ kind: "townhall", text: "which repo?" })).toMatchObject({ author: "townhall", excerpt: "which repo?" });
    expect(target(ask)).toMatchObject({ author: "fix-login", excerpt: "commit? (y/n)" });
    expect(target({ kind: "repoAsk", question: "shop or admin?", candidates: [], offerAll: false })).toMatchObject({
      author: "townhall",
      excerpt: "shop or admin?",
    });
    expect(target({ kind: "agent", agent: "fix-login", event: "failed", text: "tests red" })).toMatchObject({
      author: "fix-login",
      excerpt: "tests red",
    });
    expect(target({ kind: "agent", agent: "fix-login", event: "ended", text: "" })).toMatchObject({ excerpt: "ended" });
    expect(target({ kind: "event", text: "task fix ended" })).toMatchObject({ author: "task event", excerpt: "task fix ended" });
    expect(target({ kind: "handoff", task: "t", from: "shipper", to: "coder", reason: "lint fails" })).toMatchObject({
      author: "shipper",
      excerpt: "lint fails",
    });
    expect(target({ kind: "error", message: "claude exited with 1" })).toMatchObject({
      author: "townhall",
      excerpt: "claude exited with 1",
    });
  });

  it("has nothing to reply to on repo notes and task blocks", () => {
    expect(target({ kind: "repo", repo: "shop", baseBranch: "main", rule: "only", reason: "" })).toBeNull();
    expect(target({ kind: "tasksCreated", tasks: ["a"] })).toBeNull();
  });

  it("joins lines and cuts a long text", () => {
    const joined = target({ kind: "townhall", text: "first\n\n  second   line" });
    expect(joined?.excerpt).toBe("first second line");
    const cut = target({ kind: "townhall", text: "x".repeat(300) });
    expect(cut?.excerpt).toBe(`${"x".repeat(120)}…`);
  });
});

describe("repoReason", () => {
  it("uses the daemon's reason, else one per rule", () => {
    const repo = (rule: "named" | "code" | "only" | "picked" | "linked", reason = "") =>
      repoReason({ kind: "repo", repo: "shop", baseBranch: "main", rule, reason });
    expect(repo("code", "it has src/a.ts")).toBe("it has src/a.ts");
    expect(repo("named")).toBe("you named it");
    expect(repo("code")).toBe("found in its code");
    expect(repo("only")).toBe("only repo");
    expect(repo("picked")).toBe("you picked it");
    expect(repo("linked")).toBe("set by hand");
  });
});

describe("isNeedsYouOpen", () => {
  it("is open until the agent got an answer", () => {
    const entries = [anEntry(1, ask)];
    const tasks = [aTask({ name: "fix-login", state: "needsYou" })];
    expect(isNeedsYouOpen(entries, 0, tasks)).toBe(true);
    const answered = [...entries, anEntry(2, { kind: "agent", agent: "fix-login", event: "sent", text: "y" })];
    expect(isNeedsYouOpen(answered, 0, tasks)).toBe(false);
  });

  it("is closed once a newer question came or the task moved on", () => {
    expect(isNeedsYouOpen([anEntry(1, ask), anEntry(2, ask)], 0, [])).toBe(false);
    expect(isNeedsYouOpen([anEntry(1, ask), anEntry(2, ask)], 1, [])).toBe(true);
    expect(isNeedsYouOpen([anEntry(1, ask)], 0, [aTask({ name: "fix-login", state: "working" })])).toBe(false);
  });

  it("ignores answers to other agents", () => {
    const entries = [anEntry(1, ask), anEntry(2, { kind: "agent", agent: "other", event: "sent", text: "y" })];
    expect(isNeedsYouOpen(entries, 0, [])).toBe(true);
  });
});

describe("isRepoAskOpen", () => {
  it("is open until you write again", () => {
    const repoAsk = anEntry(2, { kind: "repoAsk", question: "which?", candidates: ["a", "b"], offerAll: true });
    expect(isRepoAskOpen([aYouEntry(1, "hi"), repoAsk], 1)).toBe(true);
    expect(isRepoAskOpen([aYouEntry(1, "hi"), repoAsk, aYouEntry(3, "a")], 1)).toBe(false);
  });
});

describe("askChoices", () => {
  it("offers both for two candidates and all for more", () => {
    expect(askChoices(["a", "b"], true)).toEqual({ candidates: ["a", "b"], all: "both" });
    expect(askChoices(["a", "b", "c"], true)).toEqual({ candidates: ["a", "b", "c"], all: "all" });
    expect(askChoices(["a", "b"], false)).toEqual({ candidates: ["a", "b"], all: null });
  });
});

describe("conversationStatus", () => {
  it("is thinking, then ended, else your turn", () => {
    expect(conversationStatus(aConversation({ status: "thinking", ended: true }))).toBe("thinking");
    expect(conversationStatus(aConversation({ ended: true }))).toBe("ended");
    expect(conversationStatus(aConversation())).toBe("your turn");
  });
});

describe("endQuestion", () => {
  it("names the conversation and the tasks that stop", () => {
    expect(endQuestion("login loop", 1)).toBe("end login loop? 1 task stops and its worktree is deleted");
    expect(endQuestion("login loop", 2)).toBe("end login loop? 2 tasks stop and their worktrees are deleted");
  });
});

describe("askAnswers", () => {
  const sent = (agent: string, text: string): EntryBody => ({ kind: "agent", agent, event: "sent", text });

  it("pairs a question with the first answer its agent got", () => {
    const entries = [anEntry(1, ask), anEntry(2, sent("other", "x")), anEntry(3, sent("fix-login", "y")), anEntry(4, sent("fix-login", "later"))];
    expect(askAnswers(entries)).toEqual(new Map([[0, 2]]));
  });

  it("gives a question no answer once a newer one from its agent came", () => {
    const entries = [anEntry(1, ask), anEntry(2, ask), anEntry(3, sent("fix-login", "y"))];
    expect(askAnswers(entries)).toEqual(new Map([[1, 2]]));
  });
});
