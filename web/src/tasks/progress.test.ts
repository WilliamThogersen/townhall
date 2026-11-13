import { describe, expect, it } from "vitest";
import { aTask } from "../api/fixtures";
import {
  allowedKinds,
  describeEvents,
  overview,
  RECENT_MS,
  taskEvents,
} from "./progress";

const NOW = Date.parse("2026-11-13T12:00:00.000Z");

describe("taskEvents", () => {
  it("reports a task that moved to merged, error or needsYou", () => {
    const prev = [
      aTask({ name: "a", state: "inCi" }),
      aTask({ name: "b", state: "working" }),
      aTask({ name: "c", state: "working" }),
      aTask({ name: "d", state: "working" }),
    ];
    const next = [
      aTask({ name: "a", state: "merged" }),
      aTask({ name: "b", state: "error" }),
      aTask({ name: "c", state: "needsYou" }),
      aTask({ name: "d", state: "inReview" }),
    ];
    expect(taskEvents(prev, next).map((e) => [e.kind, e.task.name])).toEqual([
      ["landed", "a"],
      ["failed", "b"],
      ["needsYou", "c"],
    ]);
  });

  it("ignores tasks that did not change, are new, or were already in that state", () => {
    const prev = [aTask({ name: "a", state: "merged" })];
    const next = [aTask({ name: "a", state: "merged" }), aTask({ name: "b", state: "merged" })];
    expect(taskEvents(prev, next)).toEqual([]);
  });
});

describe("allowedKinds", () => {
  it("adds one kind per step and none when off", () => {
    expect(allowedKinds("off")).toEqual([]);
    expect(allowedKinds("needsYou")).toEqual(["needsYou"]);
    expect(allowedKinds("failed")).toEqual(["needsYou", "failed"]);
    expect(allowedKinds("everything")).toEqual(["needsYou", "failed", "landed"]);
  });
});

describe("describeEvents", () => {
  it("says what happened and what is next for one event, and opens its conversation", () => {
    const landed = aTask({ name: "a", title: "fix login", state: "merged", pr: { number: 7, url: null, checks: "passed" } });
    expect(describeEvents([{ kind: "landed", task: landed }])).toEqual({
      title: "landed: fix login",
      body: "PR #7 is on main. nothing more to do.",
      conversationId: "c1",
    });
    const failed = aTask({ title: "fix login", state: "error" });
    expect(describeEvents([{ kind: "failed", task: failed }]).body).toBe("it failed. open the conversation to decide.");
    const waiting = aTask({ title: "fix login", state: "needsYou" });
    expect(describeEvents([{ kind: "needsYou", task: waiting }]).body).toBe("it needs your answer.");
  });

  it("batches several events into one line and opens the conversation only when they share one", () => {
    const events = [
      { kind: "landed" as const, task: aTask({ name: "a", conversationId: "c1" }) },
      { kind: "landed" as const, task: aTask({ name: "b", conversationId: "c1" }) },
      { kind: "needsYou" as const, task: aTask({ name: "c", conversationId: "c1" }) },
    ];
    expect(describeEvents(events)).toEqual({
      title: "townhall: 2 landed, 1 needs you",
      body: "open the conversation.",
      conversationId: "c1",
    });
    const mixed = [...events, { kind: "failed" as const, task: aTask({ name: "d", conversationId: "c2" }) }];
    const text = describeEvents(mixed);
    expect(text.title).toBe("townhall: 2 landed, 1 failed, 1 needs you");
    expect(text.conversationId).toBeNull();
  });
});

describe("overview", () => {
  it("counts running, waiting for you and recently landed tasks across conversations", () => {
    const recent = new Date(NOW - 60_000).toISOString();
    const old = new Date(NOW - RECENT_MS - 60_000).toISOString();
    const tasks = [
      aTask({ name: "a", state: "working", conversationId: "c1" }),
      aTask({ name: "b", state: "inCi", conversationId: "c2" }),
      aTask({ name: "c", state: "inReview", conversationId: "c2" }),
      aTask({ name: "d", state: "needsYou", conversationId: "c3" }),
      aTask({ name: "e", state: "queued" }),
      aTask({ name: "f", state: "merged", createdAt: recent }),
      aTask({ name: "g", state: "merged", createdAt: old }),
      aTask({ name: "h", state: "error" }),
    ];
    expect(overview(tasks, NOW)).toEqual({ running: 3, waiting: 1, landed: 1 });
  });
});
