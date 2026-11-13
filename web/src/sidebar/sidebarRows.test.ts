import { describe, expect, it } from "vitest";
import { aConversation, aTask } from "../api/fixtures";
import { buildSections, repoBlocks, visibleIds } from "./sidebarRows";

const at = (minute: number) => `2026-11-04T10:${String(minute).padStart(2, "0")}:00.000Z`;
const conversations = [
  aConversation({ id: "a", title: "alpha", updatedAt: at(9) }),
  aConversation({ id: "b", title: "beta", updatedAt: at(8) }),
  aConversation({ id: "c", title: "gamma", updatedAt: at(7) }),
  aConversation({ id: "d", title: "delta", updatedAt: at(6), ended: true }),
];
const tasks = [
  aTask({ name: "t1", conversationId: "a", repo: "shop", state: "working" }),
  aTask({ name: "t2", conversationId: "b", repo: "site", state: "working" }),
  aTask({ name: "t3", conversationId: "c", repo: "shop", state: "needsYou" }),
];

const idsOf = (rows: readonly { readonly conversation: { readonly id: string } }[]) =>
  rows.map((row) => row.conversation.id);

describe("buildSections", () => {
  it("summarizes each row from its own tasks and puts what needs you first", () => {
    const { active, older } = buildSections(conversations, tasks);
    expect(idsOf(active)).toEqual(["c", "a", "b"]);
    expect(idsOf(older)).toEqual(["d"]);
    expect(active[0]?.summary.status).toBe("t3 needs you");
    expect(active[1]?.repo).toEqual({ kind: "repo", name: "shop" });
    expect(older[0]?.repo).toEqual({ kind: "none" });
  });

  it("keeps failed red in active rows and neutral in older ones", () => {
    const failed = [
      aTask({ name: "f1", conversationId: "a", state: "error" }),
      aTask({ name: "f2", conversationId: "d", state: "error" }),
      aTask({ name: "m1", conversationId: "d", state: "merged" }),
    ];
    const { active, older } = buildSections(conversations, failed);
    expect(active.find((row) => row.conversation.id === "a")?.summary).toMatchObject({ state: "error", status: "f1 failed" });
    expect(older[0]?.summary).toEqual({ glyph: "dead", state: "ended", status: "ended · 1 merged", needsYou: false });
  });

  it("filters with the search", () => {
    const { active } = buildSections(conversations, tasks, { query: "be", textMatches: new Set(["a"]) });
    expect(idsOf(active)).toEqual(["a", "b"]);
  });
});

describe("repoBlocks and visibleIds", () => {
  const sections = buildSections(conversations, tasks);

  it("groups active rows by repo and folds a group down to the current row", () => {
    const blocks = repoBlocks(sections.active, { folded: new Set(["repo:shop"]), currentId: "a", searching: false });
    expect(blocks.map((b) => [b.label, b.count, b.open, idsOf(b.rows)])).toEqual([
      ["shop", 2, false, ["a"]],
      ["site", 1, true, ["b"]],
    ]);
    expect(visibleIds(sections, blocks)).toEqual(["a", "b", "d"]);
  });

  it("opens every group while searching", () => {
    const blocks = repoBlocks(sections.active, { folded: new Set(["repo:shop"]), currentId: null, searching: true });
    expect(blocks[0]?.rows).toHaveLength(2);
  });

  it("lists the flat order without groups", () => {
    expect(visibleIds(sections, null)).toEqual(["c", "a", "b", "d"]);
  });
});
