import { describe, expect, it } from "vitest";
import { aTask } from "../../../api/fixtures";
import type { Pr, Task, TaskState } from "../../../api/types";
import { pendingAfter, sortTasks, taskSummary, taskView } from "./taskRows";

const pr = (over: Partial<Pr> = {}): Pr => ({ number: 41, url: "https://git.test/pr/41", checks: "pending", ...over });

const named = (name: string, state: TaskState, over: Partial<Task> = {}) => aTask({ name, state, ...over });

describe("sortTasks", () => {
  it("puts what needs you first and finished work last", () => {
    const tasks = [
      named("ended-1", "ended"),
      named("merged-1", "merged"),
      named("ci-1", "inCi"),
      named("queued-1", "queued"),
      named("working-1", "working"),
      named("error-1", "error"),
      named("needs-1", "needsYou"),
    ];
    expect(sortTasks(tasks).map((t) => t.name)).toEqual([
      "needs-1",
      "error-1",
      "working-1",
      "queued-1",
      "ci-1",
      "merged-1",
      "ended-1",
    ]);
  });

  it("keeps insertion order inside a group", () => {
    const tasks = [named("w-a", "working"), named("q-a", "queued"), named("w-b", "working"), named("w-c", "working")];
    expect(sortTasks(tasks).map((t) => t.name)).toEqual(["w-a", "w-b", "w-c", "q-a"]);
  });

  it("puts in review after in ci and before merged", () => {
    const tasks = [named("m", "merged"), named("r", "inReview"), named("c", "inCi")];
    expect(sortTasks(tasks).map((t) => t.name)).toEqual(["c", "r", "m"]);
  });

  it("ranks failed checks with the errors", () => {
    const tasks = [named("w", "working"), named("ci", "inCi", { pr: pr({ checks: "failed" }) }), named("e", "error")];
    expect(sortTasks(tasks).map((t) => t.name)).toEqual(["ci", "e", "w"]);
  });

  it("returns a new array and leaves the input alone", () => {
    const tasks = [named("q", "queued"), named("n", "needsYou")];
    const sorted = sortTasks(tasks);
    expect(sorted).not.toBe(tasks);
    expect(tasks.map((t) => t.name)).toEqual(["q", "n"]);
  });
});

describe("taskView", () => {
  const view = (task: Task, siblings: readonly Task[] = [task]) => taskView(task, siblings);

  it.each<[TaskState, string, string, string | null]>([
    ["needsYou", "needs you", "waiting", "waiting"],
    ["working", "working", "working", null],
    ["queued", "queued", "queued", null],
    ["inCi", "in ci", "ci", null],
    ["inReview", "in review", "ci", null],
    ["merged", "merged", "merged", "merged"],
    ["ended", "ended", "dead", "dead"],
    ["error", "error", "error", "error"],
  ])("shows %s as %s", (state, words, glyph, tone) => {
    const v = view(named("t", state));
    expect(v.words).toBe(words);
    expect(v.glyph).toBe(glyph);
    expect(v.tone).toBe(tone);
  });

  it("names the unfinished task a queued one waits for", () => {
    const login = named("fix-login", "working");
    const cookie = named("logout-cookie", "queued", { after: ["fix-login"] });
    expect(view(cookie, [login, cookie]).words).toBe("after fix-login");
  });

  it("leaves finished tasks out of the after words", () => {
    const done = named("session-ttl", "merged");
    const ended = named("auth-tests", "ended");
    const cookie = named("logout-cookie", "queued", { after: ["session-ttl", "auth-tests"] });
    expect(view(cookie, [done, ended, cookie]).words).toBe("queued");
  });

  it("counts an after task it does not know as unfinished", () => {
    const cookie = named("logout-cookie", "queued", { after: ["fix-login", "other"] });
    expect(view(cookie).words).toBe("after fix-login, other");
  });

  it("matches after names ignoring case", () => {
    const login = named("Fix-Login", "ended");
    const cookie = named("logout-cookie", "queued", { after: ["fix-login"] });
    expect(view(cookie, [login, cookie]).words).toBe("queued");
  });

  it("says checks failed in red for a failed pull request", () => {
    const v = view(named("guard", "inCi", { pr: pr({ checks: "failed" }) }));
    expect(v).toMatchObject({ words: "checks failed", glyph: "error", tone: "error" });
  });

  it("says checks passed for a green pull request in ci", () => {
    expect(view(named("guard", "inCi", { pr: pr({ checks: "passed" }) })).words).toBe("checks passed");
  });

  it("tags an open pull request and not a merged one", () => {
    expect(view(named("a", "inReview", { pr: pr() })).prOpen).toBe(true);
    expect(view(named("b", "merged", { pr: pr() })).prOpen).toBe(false);
    expect(view(named("c", "ended", { pr: pr() })).prOpen).toBe(false);
    expect(view(named("d", "working")).prOpen).toBe(false);
  });
});

describe("pendingAfter", () => {
  it("keeps the order the task lists them in", () => {
    const task = named("x", "queued", { after: ["b", "a"] });
    expect(pendingAfter(task, [task])).toEqual(["b", "a"]);
  });
});

describe("taskSummary", () => {
  it("leaves zero parts out", () => {
    const tasks = [
      named("a", "needsYou"),
      named("b", "working"),
      named("c", "ended"),
      named("d", "ended"),
      named("e", "ended"),
    ];
    expect(taskSummary(tasks)).toBe("5 tasks · 1 needs you · 1 working · 3 ended");
  });

  it("orders the parts by attention", () => {
    const tasks = [
      named("a", "merged"),
      named("b", "inReview"),
      named("c", "inCi"),
      named("d", "queued"),
      named("e", "working"),
      named("f", "error"),
      named("g", "needsYou"),
    ];
    expect(taskSummary(tasks)).toBe(
      "7 tasks · 1 needs you · 1 error · 1 working · 1 queued · 1 in ci · 1 in review · 1 merged",
    );
  });

  it("says task for one and tasks for none", () => {
    expect(taskSummary([named("a", "working")])).toBe("1 task · 1 working");
    expect(taskSummary([])).toBe("0 tasks");
  });
});
