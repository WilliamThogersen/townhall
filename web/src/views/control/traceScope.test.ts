import { describe, expect, it } from "vitest";
import { aConversation, aTask } from "../../api/fixtures";
import type { Task } from "../../api/types";
import { conversationRepo, earliestStart, isLiveTask, scopeTrace } from "./traceScope";

const local = (h: number, m = 0, day = 4) => new Date(2026, 10, day, h, m).toISOString();
const NOW = new Date(2026, 10, 4, 21, 30).getTime();

describe("conversationRepo", () => {
  const tasks = (...repos: string[]): Task[] => repos.map((repo, i) => aTask({ name: `t${i}`, repo }));

  it("prefers the linked repo, then the most common task repo", () => {
    expect(conversationRepo(aConversation({ linkedRepo: "data-hub" }), tasks("shop"))).toBe("data-hub");
    expect(conversationRepo(aConversation(), tasks("shop", "townhall", "shop"))).toBe("shop");
  });

  it("says multiple repos on a tie and no repo without tasks", () => {
    expect(conversationRepo(aConversation(), tasks("shop", "townhall"))).toBe("multiple repos");
    expect(conversationRepo(aConversation(), [])).toBe("no repo");
  });
});

describe("isLiveTask", () => {
  it("is true while the task can still move", () => {
    const live = ["queued", "working", "needsYou", "inCi", "inReview"] as const;
    const done = ["merged", "ended", "error"] as const;
    expect(live.every((state) => isLiveTask(aTask({ state })))).toBe(true);
    expect(done.some((state) => isLiveTask(aTask({ state })))).toBe(false);
  });
});

const today = aConversation({ id: "today", createdAt: local(19), updatedAt: local(20) });
const yesterdayLive = aConversation({ id: "old-live", createdAt: local(14, 0, 3), updatedAt: local(15, 0, 3) });
const yesterdayDone = aConversation({ id: "old-done", createdAt: local(9, 0, 3), updatedAt: local(10, 0, 3) });
const thinking = aConversation({ id: "think", status: "thinking", createdAt: local(9, 0, 2), updatedAt: local(9, 0, 2) });
const early = aConversation({ id: "early", createdAt: local(8), updatedAt: local(8, 30) });

const TASKS = [
  aTask({ name: "fix-login", conversationId: "today", repo: "townhall", state: "working" }),
  aTask({ name: "ci-cache", conversationId: "today", repo: "shop", state: "merged" }),
  aTask({ name: "old-task", conversationId: "old-live", repo: "Shop", state: "inCi" }),
  aTask({ name: "done-task", conversationId: "old-done", repo: "shop", state: "merged" }),
];

const ALL = [today, yesterdayLive, yesterdayDone, thinking, early];

describe("scopeTrace", () => {
  it("keeps what ran today or is still live, oldest first, with its tasks and repo", () => {
    const scoped = scopeTrace(ALL, TASKS, { conversationId: null, repo: null }, NOW);
    expect(scoped.map((s) => s.conversation.id)).toEqual(["think", "old-live", "early", "today"]);
    const first = scoped.find((s) => s.conversation.id === "today");
    expect(first?.tasks.map((t) => t.name)).toEqual(["fix-login", "ci-cache"]);
    expect(first?.repo).toBe("multiple repos");
    expect(scoped.find((s) => s.conversation.id === "old-live")?.live).toBe(true);
    expect(scoped.find((s) => s.conversation.id === "early")?.live).toBe(false);
  });

  it("filters by repo, ignoring case, on tasks and on the linked repo", () => {
    const linked = aConversation({ id: "linked", linkedRepo: "shop", createdAt: local(20), updatedAt: local(20) });
    const scoped = scopeTrace([...ALL, linked], TASKS, { conversationId: null, repo: "SHOP" }, NOW);
    expect(scoped.map((s) => s.conversation.id)).toEqual(["old-live", "today", "linked"]);
    expect(scoped.find((s) => s.conversation.id === "today")?.tasks.map((t) => t.name)).toEqual(["ci-cache"]);
  });

  it("shows one conversation by id even when it ran before today", () => {
    const scoped = scopeTrace(ALL, TASKS, { conversationId: "old-done", repo: null }, NOW);
    expect(scoped.map((s) => s.conversation.id)).toEqual(["old-done"]);
    expect(scopeTrace(ALL, TASKS, { conversationId: "nope", repo: null }, NOW)).toEqual([]);
  });
});

describe("earliestStart", () => {
  it("clamps finished items to midnight and lets live or older ones reach back", () => {
    const todayOnly = scopeTrace([today, early], TASKS, { conversationId: null, repo: null }, NOW);
    expect(earliestStart(todayOnly, NOW)).toBe(Date.parse(local(8)));
    const withOld = scopeTrace(ALL, TASKS, { conversationId: null, repo: null }, NOW);
    expect(earliestStart(withOld, NOW)).toBe(Date.parse(local(9, 0, 2)));
    const oldDone = scopeTrace(ALL, TASKS, { conversationId: "old-done", repo: null }, NOW);
    expect(earliestStart(oldDone, NOW)).toBe(Date.parse(local(9, 0, 3)));
    const spansMidnight = aConversation({ id: "late", createdAt: local(23, 0, 3), updatedAt: local(1) });
    const late = scopeTrace([spansMidnight], [], { conversationId: null, repo: null }, NOW);
    expect(earliestStart(late, NOW)).toBe(new Date(2026, 10, 4).getTime());
    expect(earliestStart([], NOW)).toBe(NOW);
  });
});
