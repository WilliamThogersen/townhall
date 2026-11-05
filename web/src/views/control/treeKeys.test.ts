import { describe, expect, it } from "vitest";
import type { TraceRow } from "./traceRows";
import { treeAction } from "./treeKeys";

const row = (key: string, parent: string | null, expanded: boolean | null): TraceRow => ({
  key,
  kind: "task",
  depth: parent === null ? 0 : 1,
  name: key,
  meta: null,
  glyph: null,
  bar: null,
  tag: null,
  expanded,
  parent,
  href: `/x/${key}`,
});

const ROWS = [row("c1", null, true), row("t1", "c1", false), row("t2", "c1", true), row("s1", "t2", null), row("c2", null, false)];

describe("treeAction", () => {
  it("moves up, down, home and end, stopping at the ends", () => {
    expect(treeAction(ROWS, "c1", "ArrowDown")).toEqual({ kind: "focus", key: "t1" });
    expect(treeAction(ROWS, "t1", "ArrowUp")).toEqual({ kind: "focus", key: "c1" });
    expect(treeAction(ROWS, "c1", "ArrowUp")).toBeNull();
    expect(treeAction(ROWS, "c2", "ArrowDown")).toBeNull();
    expect(treeAction(ROWS, "t2", "Home")).toEqual({ kind: "focus", key: "c1" });
    expect(treeAction(ROWS, "t2", "End")).toEqual({ kind: "focus", key: "c2" });
  });

  it("unfolds a folded row with right, then steps into its first child", () => {
    expect(treeAction(ROWS, "t1", "ArrowRight")).toEqual({ kind: "toggle", key: "t1" });
    expect(treeAction(ROWS, "t2", "ArrowRight")).toEqual({ kind: "focus", key: "s1" });
    expect(treeAction(ROWS, "s1", "ArrowRight")).toBeNull();
  });

  it("folds an open row with left, else goes to its parent", () => {
    expect(treeAction(ROWS, "t2", "ArrowLeft")).toEqual({ kind: "toggle", key: "t2" });
    expect(treeAction(ROWS, "s1", "ArrowLeft")).toEqual({ kind: "focus", key: "t2" });
    expect(treeAction(ROWS, "c2", "ArrowLeft")).toBeNull();
  });

  it("opens on enter and ignores other keys and unknown rows", () => {
    expect(treeAction(ROWS, "t1", "Enter")).toEqual({ kind: "open", row: ROWS[1] });
    expect(treeAction(ROWS, "t1", "x")).toBeNull();
    expect(treeAction(ROWS, "zz", "ArrowDown")).toBeNull();
  });
});
