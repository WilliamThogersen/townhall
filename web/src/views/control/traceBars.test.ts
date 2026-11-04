import { describe, expect, it } from "vitest";
import { aConversation, anAgentInfo, aStep, aTask } from "../../api/fixtures";
import type { Step, TaskState } from "../../api/types";
import type { TimeAxis } from "./timeAxis";
import { conversationBar, slowestStep, stepBar, stepTag, taskBar, taskGlyph, taskMeta, taskTag } from "./traceBars";
import type { ScopedConversation } from "./traceScope";

const at = (h: number, m = 0, s = 0) => new Date(2026, 10, 4, h, m, s);
const iso = (h: number, m = 0, s = 0) => at(h, m, s).toISOString();
const NOW = at(11).getTime();
const AXIS: TimeAxis = { start: at(10).getTime(), end: at(11, 20).getTime(), step: 10 * 60_000, ticks: 8, now: NOW, today: true };

const entry = (over: Partial<ScopedConversation> = {}): ScopedConversation => ({
  conversation: aConversation({ createdAt: iso(10, 4), updatedAt: iso(10, 44) }),
  tasks: [],
  repo: "shop",
  live: false,
  ...over,
});

describe("conversationBar", () => {
  it("runs to now while live and says whose turn it is", () => {
    const bar = conversationBar(entry({ live: true }), AXIS);
    expect(bar).toEqual({ start: 5, len: 70, state: null, label: "56m · your turn" });
  });

  it("ends at the last activity once nothing runs, and is blue while thinking", () => {
    expect(conversationBar(entry(), AXIS).label).toBe("40m · your turn");
    const ended = entry({ conversation: aConversation({ ended: true, createdAt: iso(10), updatedAt: iso(10, 8) }) });
    expect(conversationBar(ended, AXIS)).toMatchObject({ start: 0, len: 10, label: "8m · ended" });
    const thinking = entry({ live: true, conversation: aConversation({ status: "thinking", createdAt: iso(10, 57) }) });
    expect(conversationBar(thinking, AXIS)).toMatchObject({ state: "running", label: "3m · thinking" });
  });
});

describe("task glyph, meta and tag", () => {
  it.each<[TaskState, string, string]>([
    ["queued", "queued", "queued"],
    ["working", "working", "code"],
    ["needsYou", "waiting", "review"],
    ["inCi", "ci", "review"],
    ["inReview", "alive", "review"],
    ["merged", "merged", "ship"],
    ["ended", "dead", "ended"],
    ["error", "error", "code"],
  ])("%s shows glyph %s and meta %s", (state, glyph, meta) => {
    const task = aTask({ state });
    expect(taskGlyph(task)).toBe(glyph);
    expect(taskMeta(task, null)).toBe(meta);
  });

  it("tags needs you and passed checks, nothing else", () => {
    expect(taskTag(aTask({ state: "needsYou" }))).toEqual({ text: "needs you", tone: "waiting" });
    const passed = aTask({ state: "inReview", pr: { number: 57, url: null, checks: "passed" } });
    expect(taskTag(passed)).toEqual({ text: "checks passed", tone: "ok" });
    expect(taskTag(aTask({ state: "working" }))).toBeNull();
  });
});

const steps = (...spans: [number, number, number | null, Step["result"]?][]): Step[] =>
  spans.map(([h, m, endM, result], i) =>
    aStep({
      id: `s${i}`,
      startedAt: iso(h, m),
      endedAt: endM === null ? null : iso(h, endM),
      result: result ?? (endM === null ? "running" : "ok"),
    }),
  );

describe("taskBar", () => {
  const created = iso(10, 8);

  it("draws a dashed stub at now for a queued task, naming what it waits for", () => {
    const queued = aTask({ state: "queued", after: ["fix-login"], createdAt: created });
    expect(taskBar(queued, [], null, AXIS)).toEqual({ start: 75, len: 0, state: "queued", label: "after fix-login" });
    expect(taskBar(aTask({ state: "queued" }), [], null, AXIS).label).toBe("queued");
  });

  it("runs a working task to now in blue and a waiting one in amber", () => {
    expect(taskBar(aTask({ state: "working", createdAt: created }), [], null, AXIS)).toEqual({
      start: 10,
      len: 65,
      state: "running",
      label: "52m",
    });
    expect(taskBar(aTask({ state: "needsYou", createdAt: created }), [], null, AXIS).state).toBe("waiting");
  });

  it("ends a finished task at its last step and says how it finished", () => {
    const done = steps([10, 10, 12], [10, 20, 32]);
    const merged = aTask({ state: "merged", createdAt: created, pr: { number: 38, url: null, checks: "passed" } });
    expect(taskBar(merged, done, null, AXIS)).toEqual({ start: 10, len: 30, state: null, label: "24m · merged #38" });
    expect(taskBar(aTask({ state: "ended", createdAt: created }), done, null, AXIS).label).toBe("24m · ended");
    const review = aTask({ state: "inReview", createdAt: created, pr: { number: 57, url: null, checks: "passed" } });
    expect(taskBar(review, done, null, AXIS).label).toBe("24m · in review #57");
  });

  it("keeps a task in ci running to now", () => {
    const ci = aTask({ state: "inCi", createdAt: created, pr: { number: 52, url: null, checks: "pending" } });
    expect(taskBar(ci, [], null, AXIS)).toMatchObject({ state: "running", label: "52m · in ci #52" });
    expect(taskBar(aTask({ state: "inCi", createdAt: created }), [], null, AXIS).label).toBe("52m · in ci");
  });

  it("says why a task failed", () => {
    const done = steps([10, 10, 20, "error"]);
    const failedChecks = aTask({ state: "error", createdAt: created, pr: { number: 41, url: null, checks: "failed" } });
    expect(taskBar(failedChecks, done, null, AXIS)).toMatchObject({ state: "error", label: "12m · checks failed" });
    const exited = anAgentInfo({ alive: false, state: { kind: "error", exitCode: 1 } });
    expect(taskBar(aTask({ state: "error", createdAt: created }), done, exited, AXIS).label).toBe("12m · exited 1");
    expect(taskBar(aTask({ state: "error", createdAt: created }), done, null, AXIS).label).toBe("12m · failed");
  });

  it("leaves the duration out while a finished task has no steps", () => {
    expect(taskBar(aTask({ state: "merged", createdAt: created }), [], null, AXIS)).toEqual({
      start: 10,
      len: 0,
      state: null,
      label: "merged",
    });
  });
});

describe("stepBar and stepTag", () => {
  it("draws finished, failed, running and asking steps", () => {
    const [ok, failed, running] = steps([10, 10, 12], [10, 20, 30, "error"], [10, 50, null]);
    expect(stepBar(ok as Step, AXIS)).toEqual({ start: 12.5, len: 2.5, state: null, label: "2m" });
    expect(stepBar(failed as Step, AXIS)).toMatchObject({ state: "error", label: "10m" });
    expect(stepBar(running as Step, AXIS)).toMatchObject({ state: "running", label: "10m · running" });
    const ask = aStep({ tool: "AskUserQuestion", startedAt: iso(10, 55), endedAt: null, result: "running" });
    expect(stepBar(ask, AXIS).state).toBe("waiting");
    expect(stepTag(ask, false)).toEqual({ text: "needs you", tone: "waiting" });
    expect(stepTag(failed as Step, false)).toEqual({ text: "failed", tone: "error" });
    expect(stepTag(ok as Step, true)).toEqual({ text: "slowest", tone: null });
    expect(stepTag(ok as Step, false)).toBeNull();
  });

  it("finds the slowest finished step once there are three and it stands out", () => {
    const three = steps([10, 0, 1], [10, 2, 9], [10, 10, 12], [10, 20, null]);
    expect(slowestStep(three)).toBe("s1");
    expect(slowestStep(three.slice(0, 2))).toBeNull();
    expect(slowestStep(steps([10, 0, 2], [10, 2, 4], [10, 4, 7]))).toBeNull();
  });
});
