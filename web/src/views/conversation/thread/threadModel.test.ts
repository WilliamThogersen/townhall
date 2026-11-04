import { describe, expect, it } from "vitest";
import { aConversation, anEntry, aTask, aYouEntry } from "../../../api/fixtures";
import type { EntryBody } from "../../../api/types";
import {
  askChoices,
  clockTime,
  conversationStatus,
  endQuestion,
  isNeedsYouOpen,
  isRepoAskOpen,
  repoReason,
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
  it("names the conversation and the tasks that keep running", () => {
    expect(endQuestion("login loop", 1)).toBe("end login loop? 1 task keeps running");
    expect(endQuestion("login loop", 2)).toBe("end login loop? 2 tasks keep running");
  });
});
