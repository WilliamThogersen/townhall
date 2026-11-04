import { describe, expect, it } from "vitest";
import { controlHints } from "./useControlKeys";
import { controlPath } from "./useOpenAgent";

describe("controlPath", () => {
  it("adds the open agent as a query parameter", () => {
    expect(controlPath()).toBe("/townhall");
    expect(controlPath(null)).toBe("/townhall");
    expect(controlPath("")).toBe("/townhall");
    expect(controlPath("fix-login")).toBe("/townhall?agent=fix-login");
    expect(controlPath("a b")).toBe("/townhall?agent=a%20b");
  });
});

describe("controlHints", () => {
  it("only lists keys that work right now", () => {
    expect(controlHints(false, false)).toEqual([]);
    expect(controlHints(true, false).map((h) => h.keys)).toEqual(["↑↓", "⏎"]);
    expect(controlHints(true, true).map((h) => h.keys)).toEqual(["↑↓", "⏎", "esc"]);
    expect(controlHints(false, true).map((h) => h.keys)).toEqual(["esc"]);
  });
});
