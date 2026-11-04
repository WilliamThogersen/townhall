import { describe, expect, it } from "vitest";
import { aConversation, aTask } from "../api/fixtures";
import { groupByRepo, inferRepo, repoLabel, repoOfConversation, rowRepoText } from "./conversationRepo";

const onRepos = (...repos: string[]) => repos.map((repo, i) => aTask({ name: `t${i}`, repo }));

describe("repoOfConversation", () => {
  it("takes the repo set from the row menu first", () => {
    const c = aConversation({ linkedRepo: "site" });
    expect(repoOfConversation(c, onRepos("shop", "shop"))).toEqual({ kind: "repo", name: "site" });
  });

  it("takes the most common repo among its tasks", () => {
    expect(repoOfConversation(aConversation(), onRepos("shop", "site", "Shop"))).toEqual({ kind: "repo", name: "shop" });
  });

  it("says multiple repos on a tie and no repo without tasks", () => {
    expect(inferRepo(onRepos("shop", "site"))).toEqual({ kind: "multiple" });
    expect(inferRepo([])).toEqual({ kind: "none" });
  });

  it("labels each kind for a group and for a row", () => {
    expect(repoLabel({ kind: "repo", name: "shop" })).toBe("shop");
    expect(repoLabel({ kind: "multiple" })).toBe("multiple repos");
    expect(repoLabel({ kind: "none" })).toBe("no repo");
    expect(rowRepoText({ kind: "none" })).toBe("");
    expect(rowRepoText({ kind: "multiple" })).toBe("multiple repos");
  });
});

describe("groupByRepo", () => {
  it("keeps the list order inside and between groups, fixed groups last", () => {
    const rows = [
      aConversation({ id: "a" }),
      aConversation({ id: "b" }),
      aConversation({ id: "c" }),
      aConversation({ id: "d" }),
      aConversation({ id: "e" }),
    ];
    const repos = new Map([
      ["a", { kind: "none" } as const],
      ["b", { kind: "repo", name: "shop" } as const],
      ["c", { kind: "multiple" } as const],
      ["d", { kind: "repo", name: "townhall" } as const],
      ["e", { kind: "repo", name: "Shop" } as const],
    ]);
    const groups = groupByRepo(rows, (c) => repos.get(c.id) ?? { kind: "none" });
    expect(groups.map((g) => [g.label, g.items.map((c) => c.id)])).toEqual([
      ["shop", ["b", "e"]],
      ["townhall", ["d"]],
      ["multiple repos", ["c"]],
      ["no repo", ["a"]],
    ]);
    expect(new Set(groups.map((g) => g.key)).size).toBe(4);
  });
});
