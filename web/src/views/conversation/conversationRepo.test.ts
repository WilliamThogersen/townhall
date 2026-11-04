import { describe, expect, it } from "vitest";
import { aConversation, aDetail, anEntry, aRepo, aTask } from "../../api/fixtures";
import { conversationRepo, repoText } from "./conversationRepo";

const repos = [aRepo({ name: "shop", baseBranch: "develop" }), aRepo({ name: "townhall", baseBranch: "main" })];

describe("conversationRepo", () => {
  it("prefers the repo set by hand, with its base branch", () => {
    const detail = aDetail({
      conversation: aConversation({ linkedRepo: "Townhall" }),
      tasks: [aTask({ name: "a", repo: "shop" })],
    });
    expect(conversationRepo(detail, repos)).toEqual({ kind: "repo", name: "Townhall", branch: "main" });
  });

  it("names a hand-set repo without a branch while the repos are unknown", () => {
    const detail = aDetail({ conversation: aConversation({ linkedRepo: "shop" }) });
    expect(conversationRepo(detail, null)).toEqual({ kind: "repo", name: "shop", branch: null });
  });

  it("takes the most common repo among the tasks", () => {
    const detail = aDetail({
      tasks: [
        aTask({ name: "a", repo: "shop", baseBranch: "develop" }),
        aTask({ name: "b", repo: "townhall", baseBranch: "main" }),
        aTask({ name: "c", repo: "shop", baseBranch: "develop" }),
      ],
    });
    expect(conversationRepo(detail, repos)).toEqual({ kind: "repo", name: "shop", branch: "develop" });
  });

  it("calls a tie multiple repos", () => {
    const detail = aDetail({
      tasks: [aTask({ name: "a", repo: "shop" }), aTask({ name: "b", repo: "townhall" })],
    });
    expect(conversationRepo(detail, repos)).toEqual({ kind: "multiple" });
  });

  it("falls back to the latest repo townhall picked before any task", () => {
    const detail = aDetail({
      entries: [
        anEntry(1, { kind: "repo", repo: "shop", baseBranch: "develop", rule: "code", reason: "it has x" }),
        anEntry(2, { kind: "repo", repo: "townhall", baseBranch: "main", rule: "named", reason: "" }),
      ],
    });
    expect(conversationRepo(detail, repos)).toEqual({ kind: "repo", name: "townhall", branch: "main" });
  });

  it("has no repo otherwise", () => {
    expect(conversationRepo(aDetail(), repos)).toEqual({ kind: "none" });
  });
});

describe("repoText", () => {
  it("writes the repo with its branch, or what is known", () => {
    expect(repoText({ kind: "repo", name: "shop", branch: "develop" })).toBe("~ shop · develop");
    expect(repoText({ kind: "repo", name: "shop", branch: null })).toBe("~ shop");
    expect(repoText({ kind: "multiple" })).toBe("multiple repos");
    expect(repoText({ kind: "none" })).toBe("no repo");
  });
});
