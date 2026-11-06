import { describe, expect, it } from "vitest";
import { anAgentTypeSummary, aStep, aTask } from "../api/fixtures";
import type { TaskState } from "../api/types";
import { LOOP_STAGES, loopSummary, stageOf, stepKind, stepVerb } from "./stage";

const bash = (summary: string) => aStep({ tool: "Bash", summary });

describe("stepKind", () => {
  it.each([
    ["Read", "src/a.ts", "read"],
    ["NotebookRead", "a.ipynb", "read"],
    ["Grep", "session.expired", "search"],
    ["Glob", "**/*.ts", "search"],
    ["WebFetch", "https://x", "search"],
    ["Edit", "src/a.ts", "edit"],
    ["MultiEdit", "src/a.ts", "edit"],
    ["Write", "src/a.test.ts", "edit"],
    ["AskUserQuestion", "which repo?", "ask"],
    ["TodoWrite", "3 todos", "other"],
  ])("%s is %s", (tool, summary, kind) => {
    expect(stepKind(aStep({ tool, summary }))).toBe(kind);
  });

  it.each([
    ["npm test -- auth --watch=false", "test"],
    ["npm run test", "test"],
    ["pnpm test", "test"],
    ["npx playwright test e2e/smoke.spec.ts", "test"],
    ["npx vitest run", "test"],
    ["cargo test --workspace", "test"],
    ["go test ./...", "test"],
    ["pytest -q", "test"],
    ["git commit -am \"test: cover the expiry\"", "commit"],
    ["git add . && git commit -m fix", "commit"],
    ["cat src/a.test.ts", "run"],
    ["k6 run load/search.js", "run"],
    ["gh pr create --fill", "run"],
  ])("Bash %s is %s", (summary, kind) => {
    expect(stepKind(bash(summary))).toBe(kind);
  });
});

describe("stepVerb", () => {
  it("names a step by what it does, else by its tool in lowercase", () => {
    expect(stepVerb(aStep({ tool: "Read" }))).toBe("read");
    expect(stepVerb(bash("npm test"))).toBe("run tests");
    expect(stepVerb(bash("git commit -m x"))).toBe("commit");
    expect(stepVerb(bash("ls"))).toBe("run");
    expect(stepVerb(aStep({ tool: "AskUserQuestion" }))).toBe("ask");
    expect(stepVerb(aStep({ tool: "TodoWrite" }))).toBe("todowrite");
  });
});

describe("stageOf", () => {
  const at = (state: TaskState, over = {}) => aTask({ state, ...over });

  it("maps the task state when it says enough", () => {
    expect(stageOf(at("queued"), null)).toBe("plan");
    expect(stageOf(at("needsYou"), null)).toBe("review");
    expect(stageOf(at("inCi"), null)).toBe("review");
    expect(stageOf(at("inReview"), null)).toBe("review");
    expect(stageOf(at("merged"), null)).toBe("ship");
    expect(stageOf(at("ended"), null)).toBeNull();
  });

  it("reads a working task's stage from its latest step", () => {
    expect(stageOf(at("working"), null)).toBe("code");
    expect(stageOf(at("working"), aStep({ tool: "Edit" }))).toBe("code");
    expect(stageOf(at("working"), bash("npm test"))).toBe("test");
    expect(stageOf(at("working"), aStep({ tool: "AskUserQuestion" }))).toBe("review");
  });

  it("keeps a read-only planner in plan while it reads and searches", () => {
    const planner = anAgentTypeSummary({ name: "planner", permissionMode: "plan" });
    expect(stageOf(at("working", { agent: "planner" }), aStep({ tool: "Read" }), planner)).toBe("plan");
    expect(stageOf(at("working", { agent: "planner" }), aStep({ tool: "Grep" }), planner)).toBe("plan");
    expect(stageOf(at("working", { agent: "planner" }), null, planner)).toBe("plan");
  });

  it("treats any plan-mode type as planning, whatever its name", () => {
    const scout = anAgentTypeSummary({ name: "scout", permissionMode: "plan" });
    expect(stageOf(at("working", { agent: "scout" }), aStep({ tool: "Glob" }), scout)).toBe("plan");
  });

  it("maps a reviewer type to review and a planner by name when the type is not loaded", () => {
    const reviewer = anAgentTypeSummary({ name: "reviewer", permissionMode: "default" });
    expect(stageOf(at("working", { agent: "reviewer" }), aStep({ tool: "Read" }), reviewer)).toBe("review");
    expect(stageOf(at("working", { agent: "planner" }), aStep({ tool: "Read" }), null)).toBe("plan");
  });

  it("keeps the step-based stage for builder-like types", () => {
    const builder = anAgentTypeSummary({ name: "builder", permissionMode: "acceptEdits" });
    expect(stageOf(at("working"), aStep({ tool: "Read" }), builder)).toBe("code");
    expect(stageOf(at("working"), bash("npm test"), builder)).toBe("test");
    expect(stageOf(at("working"), aStep({ tool: "AskUserQuestion" }), builder)).toBe("review");
  });

  it("lets the task state win over the agent type", () => {
    const planner = anAgentTypeSummary({ name: "planner", permissionMode: "plan" });
    expect(stageOf(at("merged", { agent: "planner" }), null, planner)).toBe("ship");
    expect(stageOf(at("inReview", { agent: "planner" }), null, planner)).toBe("review");
    expect(stageOf(at("ended", { agent: "planner" }), null, planner)).toBeNull();
  });

  it("puts a failed task where it failed", () => {
    const failedChecks = { pr: { number: 41, url: null, checks: "failed" as const } };
    expect(stageOf(at("error", failedChecks), bash("npm test"))).toBe("review");
    expect(stageOf(at("error"), bash("npx playwright test"))).toBe("test");
    expect(stageOf(at("error"), null)).toBe("code");
  });
});

describe("loopSummary", () => {
  it("counts tasks per stage, brightens stages with unfinished tasks and counts needs you", () => {
    const tasks = [
      aTask({ name: "a", state: "queued" }),
      aTask({ name: "b", state: "working" }),
      aTask({ name: "c", state: "working" }),
      aTask({ name: "d", state: "needsYou" }),
      aTask({ name: "e", state: "merged" }),
      aTask({ name: "f", state: "error" }),
      aTask({ name: "g", state: "ended" }),
    ];
    const latest = (name: string) => (name === "c" ? bash("npm test") : null);
    const summary = loopSummary(tasks, latest);
    expect(summary.stages.map((s) => s.stage)).toEqual(LOOP_STAGES);
    expect(summary.stages.map((s) => [s.stage, s.count, s.active])).toEqual([
      ["plan", 1, true],
      ["code", 2, true],
      ["test", 1, true],
      ["review", 1, true],
      ["ship", 1, false],
    ]);
    expect(summary.needsYou).toBe(1);
  });

  it("reads task states alone without a step lookup", () => {
    const states: readonly TaskState[] = ["queued", "working", "needsYou", "error", "inCi", "inReview", "merged", "ended"];
    const summary = loopSummary(states.map((state, i) => aTask({ name: `t${i}`, state })));
    expect(summary.stages.map((s) => [s.stage, s.count])).toEqual([
      ["plan", 1],
      ["code", 2],
      ["test", 0],
      ["review", 3],
      ["ship", 1],
    ]);
    expect(summary.needsYou).toBe(1);
  });

  it("counts a running planner in plan using the agent types it is given", () => {
    const types = [anAgentTypeSummary({ name: "planner", permissionMode: "plan" })];
    const tasks = [aTask({ name: "p", state: "working", agent: "planner" }), aTask({ name: "b", state: "working" })];
    const reading = () => aStep({ tool: "Read" });
    const summary = loopSummary(tasks, reading, types);
    expect(summary.stages.map((s) => [s.stage, s.count])).toEqual([
      ["plan", 1],
      ["code", 1],
      ["test", 0],
      ["review", 0],
      ["ship", 0],
    ]);
  });

  it("is all zeros for no tasks", () => {
    const summary = loopSummary([], () => null);
    expect(summary.stages.every((s) => s.count === 0 && !s.active)).toBe(true);
    expect(summary.needsYou).toBe(0);
  });
});
