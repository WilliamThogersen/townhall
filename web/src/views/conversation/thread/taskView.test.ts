import { describe, expect, it } from "vitest";
import { aTask } from "../../../api/fixtures";
import type { TaskState } from "../../../api/types";
import { taskView } from "../session/taskRows";
import { needsYouCount, taskBlockHead, taskGlyph, taskWords, unfinishedCount } from "./taskView";

describe("taskGlyph", () => {
  it("maps every task state to a status glyph", () => {
    const states: readonly TaskState[] = ["queued", "working", "needsYou", "inCi", "inReview", "merged", "ended", "error"];
    expect(states.map(taskGlyph)).toEqual(["queued", "working", "waiting", "ci", "ci", "merged", "dead", "error"]);
  });
});

describe("taskWords", () => {
  it("says what each state means", () => {
    const words = (state: TaskState) => taskWords(aTask({ state }), []);
    expect(words("working")).toBe("working");
    expect(words("needsYou")).toBe("needs you");
    expect(words("inCi")).toBe("in ci");
    expect(words("inReview")).toBe("in review");
    expect(words("merged")).toBe("merged");
    expect(words("ended")).toBe("ended");
    expect(words("error")).toBe("failed");
    expect(words("queued")).toBe("queued");
  });

  it("names failed checks", () => {
    const pr = { number: 41, url: null, checks: "failed" as const };
    expect(taskWords(aTask({ state: "error", pr }), [])).toBe("checks failed");
    expect(taskWords(aTask({ state: "inCi", pr }), [])).toBe("checks failed");
  });

  it("uses the session panel's words for every task", () => {
    const pr = (checks: "passed" | "failed") => ({ number: 41, url: null, checks });
    const tasks = [
      aTask({ name: "Fix-Login", state: "working" }),
      aTask({ name: "broken", state: "error" }),
      aTask({ name: "green", state: "inCi", pr: pr("passed") }),
      aTask({ name: "red", state: "error", pr: pr("failed") }),
      aTask({ name: "logout", state: "queued", after: ["fix-login"] }),
    ];
    expect(tasks.map((task) => taskWords(task, tasks))).toEqual(tasks.map((task) => taskView(task, tasks).words));
    expect(tasks.map((task) => taskWords(task, tasks))).toEqual([
      "working",
      "failed",
      "checks passed",
      "checks failed",
      "after fix-login",
    ]);
  });

  it("names the unfinished tasks a queued task waits for", () => {
    const tasks = [
      aTask({ name: "fix-login", state: "working" }),
      aTask({ name: "done-one", state: "merged" }),
    ];
    const queued = aTask({ name: "logout", state: "queued", after: ["fix-login", "done-one", "gone"] });
    expect(taskWords(queued, tasks)).toBe("after fix-login, gone");
    expect(taskWords(aTask({ state: "queued", after: ["done-one"] }), tasks)).toBe("queued");
  });
});

describe("counts", () => {
  const tasks = [
    aTask({ name: "a", state: "needsYou" }),
    aTask({ name: "b", state: "working" }),
    aTask({ name: "c", state: "merged" }),
    aTask({ name: "d", state: "error" }),
    aTask({ name: "e", state: "queued" }),
  ];

  it("counts the tasks that keep running", () => {
    expect(unfinishedCount(tasks)).toBe(3);
  });

  it("counts the tasks that need you", () => {
    expect(needsYouCount(tasks)).toBe(1);
  });
});

describe("taskBlockHead", () => {
  it("names one repo with its branch", () => {
    const tasks = [aTask({ name: "a", repo: "townhall", baseBranch: "main" }), aTask({ name: "b", repo: "townhall" })];
    expect(taskBlockHead(tasks, 2)).toBe("2 tasks for townhall · main");
  });

  it("lists several repos", () => {
    const tasks = [aTask({ name: "a", repo: "shop" }), aTask({ name: "b", repo: "townhall" })];
    expect(taskBlockHead(tasks, 3)).toBe("3 tasks for shop, townhall");
  });

  it("counts one task without a known repo", () => {
    expect(taskBlockHead([], 1)).toBe("1 task");
  });
});
