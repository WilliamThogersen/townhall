import { describe, expect, it } from "vitest";
import { anAgentInfo, aTask } from "../../../api/fixtures";
import type { Pr, TaskState } from "../../../api/types";
import { findTaskAgent, noTerminalText } from "./terminalModel";

const pr: Pr = { number: 41, url: null, checks: "passed" };

describe("noTerminalText", () => {
  it("names what a queued task waits for", () => {
    const login = aTask({ name: "fix-login", state: "working" });
    const tests = aTask({ name: "auth-tests", state: "queued" });
    const cookie = aTask({ name: "logout-cookie", state: "queued", after: ["fix-login", "auth-tests"] });
    expect(noTerminalText(cookie, [login, tests, cookie])).toBe("queued: waits for fix-login and auth-tests");
    expect(noTerminalText(aTask({ state: "queued", after: ["a", "b", "c"] }), [])).toBe(
      "queued: waits for a, b and c",
    );
  });

  it("says a queued task without open dependencies waits for a slot", () => {
    expect(noTerminalText(aTask({ state: "queued" }), [])).toBe("queued: waits for a free slot");
  });

  it.each<[TaskState, Pr | null, string]>([
    ["merged", pr, "merged: #41"],
    ["merged", null, "merged"],
    ["inCi", pr, "in ci: #41"],
    ["inReview", pr, "in review: #41"],
    ["inReview", null, "in review"],
    ["ended", null, "ended: no terminal left"],
    ["error", null, "error: no terminal left"],
    ["working", null, "working: no terminal yet"],
    ["needsYou", null, "needs you: no terminal yet"],
  ])("says %s with %o as %s", (state, taskPr, text) => {
    expect(noTerminalText(aTask({ state, pr: taskPr }), [])).toBe(text);
  });
});

describe("findTaskAgent", () => {
  it("finds the agent by name and repo, ignoring case", () => {
    const other = anAgentInfo({ name: "fix-login", repo: "townhall" });
    const own = anAgentInfo({ name: "Fix-Login", repo: "Shop" });
    expect(findTaskAgent([other, own], aTask({ name: "fix-login", repo: "shop" }))).toBe(own);
  });

  it("falls back to the name alone", () => {
    const only = anAgentInfo({ name: "fix-login", repo: "elsewhere" });
    expect(findTaskAgent([only], aTask({ name: "fix-login", repo: "shop" }))).toBe(only);
  });

  it("is null when no agent has the name", () => {
    expect(findTaskAgent([anAgentInfo({ name: "other" })], aTask())).toBeNull();
  });
});
