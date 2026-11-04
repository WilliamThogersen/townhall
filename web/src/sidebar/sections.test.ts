import { describe, expect, it } from "vitest";
import { aConversation } from "../api/fixtures";
import type { Conversation } from "../api/types";
import { foldParts, matchesTitle, normalizeQuery, splitSections } from "./sections";

const at = (minute: number) => `2026-11-04T10:${String(minute).padStart(2, "0")}:00.000Z`;
const c = (id: string, minute: number, over: Partial<Conversation> = {}) =>
  aConversation({ id, title: id, updatedAt: at(minute), ...over });
const ids = (rows: readonly Conversation[]) => rows.map((row) => row.id);
const nobody = () => false;

describe("normalizeQuery and matchesTitle", () => {
  it("ignore case and runs of whitespace", () => {
    expect(normalizeQuery("  Login \n  LOOP ")).toBe("login loop");
    expect(matchesTitle("the login\tloop again", "login loop")).toBe(true);
    expect(matchesTitle("ÆBLEKAGE", normalizeQuery("æble"))).toBe(true);
    expect(matchesTitle("checkout", "login")).toBe(false);
  });
});

describe("splitSections", () => {
  const rows = [c("old", 1), c("new", 9), c("ended-old", 2, { ended: true }), c("ended-new", 8, { ended: true }), c("mid", 5)];

  it("puts ended conversations under older, newest activity first in both", () => {
    const { active, older } = splitSections(rows, nobody);
    expect(ids(active)).toEqual(["new", "mid", "old"]);
    expect(ids(older)).toEqual(["ended-new", "ended-old"]);
  });

  it("puts active rows that need you first", () => {
    const { active, older } = splitSections(rows, (row) => row.id === "old" || row.id === "ended-old");
    expect(ids(active)).toEqual(["old", "new", "mid"]);
    expect(ids(older)).toEqual(["ended-new", "ended-old"]);
  });

  it("filters by title at once and by message matches when they come", () => {
    const filter = { query: "NE", textMatches: new Set(["mid"]) };
    const { active, older } = splitSections(rows, nobody, filter);
    expect(ids(active)).toEqual(["new", "mid"]);
    expect(ids(older)).toEqual(["ended-new"]);
  });

  it("shows everything for a blank query", () => {
    const { active } = splitSections(rows, nobody, { query: "   ", textMatches: new Set() });
    expect(active).toHaveLength(3);
  });
});

describe("foldParts", () => {
  const rows = [c("a", 1), c("b", 2), c("c", 3)];

  it("splits the rows around the current one", () => {
    const parts = foldParts(rows, "b");
    expect([ids(parts.before), parts.current?.id, ids(parts.after)]).toEqual([["a"], "b", ["c"]]);
  });

  it("keeps every row before when none is current", () => {
    const parts = foldParts(rows, null);
    expect([ids(parts.before), parts.current, ids(parts.after)]).toEqual([["a", "b", "c"], null, []]);
  });
});
