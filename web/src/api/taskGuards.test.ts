import { describe, expect, it } from "vitest";
import { aTask } from "./fixtures";
import { parsePr, parseTask } from "./taskGuards";

describe("task guards", () => {
  it("reads a task with and without a pr", () => {
    expect(parseTask(aTask())).toEqual(aTask());
    const reviewed = aTask({ state: "inReview", pr: { number: 41, url: null, checks: "failed" } });
    expect(parseTask(reviewed)).toEqual(reviewed);
  });

  it("reads why a queued task is held back", () => {
    const held = aTask({ blocked: "`t1` (reviewer) reported REJECT" });
    expect(parseTask(held).blocked).toBe("`t1` (reviewer) reported REJECT");
    expect(parseTask(aTask()).blocked).toBeNull();
    expect(() => parseTask({ ...aTask(), blocked: 3 })).toThrow("task.blocked");
    expect(parseTask({ ...aTask(), blocked: undefined }).blocked).toBeNull();
  });

  it("reads the branch a pull request targets", () => {
    expect(parseTask(aTask()).prBase).toBeNull();
    expect(parseTask(aTask({ prBase: "main" })).prBase).toBe("main");
    expect(parseTask({ ...aTask(), prBase: undefined }).prBase).toBeNull();
    expect(() => parseTask({ ...aTask(), prBase: 3 })).toThrow("task.prBase");
  });

  it("refuses broken tasks and prs", () => {
    expect(() => parseTask({ ...aTask(), after: [3] })).toThrow("task.after[0]");
    expect(() => parseTask({ ...aTask(), agent: undefined })).toThrow("task.agent");
    expect(parseTask(aTask({ agent: "reviewer" })).agent).toBe("reviewer");
    expect(() => parsePr({ number: 1, url: null, checks: "green" })).toThrow("task.pr.checks");
    expect(() => parsePr({ number: Number.NaN, url: null, checks: "none" })).toThrow("task.pr.number");
  });
});
