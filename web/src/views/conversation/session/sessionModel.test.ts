import { describe, expect, it } from "vitest";
import { aConversation, aDetail, anEntry, aTask, aYouEntry } from "../../../api/fixtures";
import type { RepoRule, TaskState, ThreadEntry } from "../../../api/types";
import { contextView, formatTokens } from "./contextUsage";
import { loopCounts } from "./loopStages";
import { repoLine } from "./repoLine";

const tasksIn = (...states: TaskState[]) => states.map((state, i) => aTask({ name: `t${i}`, state }));

describe("loopCounts", () => {
  it("maps each task state to a stage", () => {
    const loop = loopCounts(
      tasksIn("queued", "working", "needsYou", "error", "inCi", "inReview", "inReview", "merged", "ended"),
    );
    expect(loop.stages).toEqual([
      { stage: "plan", count: 1 },
      { stage: "code", count: 3 },
      { stage: "test", count: 1 },
      { stage: "review", count: 2 },
      { stage: "ship", count: 1 },
    ]);
  });

  it("counts what needs you on its own", () => {
    expect(loopCounts(tasksIn("needsYou", "needsYou", "working")).needsYou).toBe(2);
    expect(loopCounts(tasksIn("working")).needsYou).toBe(0);
  });

  it("shows every stage at zero without tasks", () => {
    const loop = loopCounts([]);
    expect(loop.stages.map((s) => s.count)).toEqual([0, 0, 0, 0, 0]);
    expect(loop.needsYou).toBe(0);
  });
});

const repoEntry = (id: number, repo: string, rule: RepoRule, reason: string, baseBranch = "main"): ThreadEntry =>
  anEntry(id, { kind: "repo", repo, baseBranch, rule, reason });

describe("repoLine", () => {
  it("uses the newest repo entry and its reason", () => {
    const detail = aDetail({
      entries: [
        repoEntry(2, "shop", "named", "you named it", "develop"),
        aYouEntry(3, "now the search"),
        repoEntry(4, "townhall", "code", "it has src/search/query.ts"),
        aYouEntry(5, "thanks"),
      ],
    });
    expect(repoLine(detail)).toEqual({ label: "~ townhall · main", reason: "it has src/search/query.ts" });
  });

  it("picks the highest id, not the last in the list", () => {
    const detail = aDetail({
      entries: [repoEntry(9, "townhall", "only", "only repo"), repoEntry(2, "shop", "named", "you named it")],
    });
    expect(repoLine(detail)?.label).toBe("~ townhall · main");
  });

  it.each<[RepoRule, string]>([
    ["named", "you named it"],
    ["code", "found in its code"],
    ["only", "the only repo"],
    ["picked", "you picked it"],
    ["linked", "set by hand"],
  ])("says why for a %s entry without a reason", (rule, reason) => {
    const detail = aDetail({ entries: [repoEntry(1, "shop", rule, "  ")] });
    expect(repoLine(detail)?.reason).toBe(reason);
  });

  it("says set by hand when the linked repo differs from the newest entry", () => {
    const detail = aDetail({
      conversation: aConversation({ linkedRepo: "shop" }),
      entries: [repoEntry(1, "townhall", "named", "you named it")],
      tasks: [aTask({ repo: "Shop", baseBranch: "develop" })],
    });
    expect(repoLine(detail)).toEqual({ label: "~ shop · develop", reason: "set by hand" });
  });

  it("keeps the entry when it names the linked repo", () => {
    const detail = aDetail({
      conversation: aConversation({ linkedRepo: "Townhall" }),
      entries: [repoEntry(1, "townhall", "linked", "you linked it")],
    });
    expect(repoLine(detail)).toEqual({ label: "~ townhall · main", reason: "you linked it" });
  });

  it("leaves the branch out of a linked repo no task has used", () => {
    const detail = aDetail({ conversation: aConversation({ linkedRepo: "shop" }) });
    expect(repoLine(detail)).toEqual({ label: "~ shop", reason: "set by hand" });
  });

  it("is null before townhall picked a repo", () => {
    expect(repoLine(aDetail({ entries: [aYouEntry(1, "hi")] }))).toBeNull();
  });
});

describe("formatTokens", () => {
  it.each<[number, string]>([
    [0, "0"],
    [999, "999"],
    [1000, "1k"],
    [56_400, "56k"],
    [200_000, "200k"],
    [999_499, "999k"],
    [1_000_000, "1m"],
    [1_250_000, "1.3m"],
  ])("writes %d as %s", (n, text) => {
    expect(formatTokens(n)).toBe(text);
  });
});

describe("contextView", () => {
  it("is null before the session has any context", () => {
    expect(contextView(null)).toBeNull();
  });

  it("gives the percent, the words and a normal level under the threshold", () => {
    expect(contextView({ used: 56_000, limit: 200_000 })).toEqual({
      percent: 28,
      high: false,
      words: "56k of 200k tokens",
    });
  });

  it("turns high at 80 percent", () => {
    expect(contextView({ used: 159_000, limit: 200_000 })?.high).toBe(false);
    expect(contextView({ used: 160_000, limit: 200_000 })?.high).toBe(true);
  });

  it("keeps the percent between 0 and 100", () => {
    expect(contextView({ used: 300_000, limit: 200_000 })?.percent).toBe(100);
    expect(contextView({ used: 10, limit: 0 })?.percent).toBe(0);
    expect(contextView({ used: -5, limit: 100 })?.percent).toBe(0);
  });
});
