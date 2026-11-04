import { describe, expect, it } from "vitest";
import { aTask } from "./fixtures";
import { parsePr, parseTask } from "./taskGuards";

describe("task guards", () => {
  it("reads a task with and without a pr", () => {
    expect(parseTask(aTask())).toEqual(aTask());
    const reviewed = aTask({ state: "inReview", pr: { number: 41, url: null, checks: "failed" } });
    expect(parseTask(reviewed)).toEqual(reviewed);
  });

  it("refuses broken tasks and prs", () => {
    expect(() => parseTask({ ...aTask(), after: [3] })).toThrow("task.after[0]");
    expect(() => parsePr({ number: 1, url: null, checks: "green" })).toThrow("task.pr.checks");
    expect(() => parsePr({ number: Number.NaN, url: null, checks: "none" })).toThrow("task.pr.number");
  });
});
