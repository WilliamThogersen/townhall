import { describe, expect, it } from "vitest";
import type { AgentRun } from "../../api/types";
import { RUNS_SHOWN, runModel, shownRuns, showsModelColumn } from "./agentRuns";

const run = (task: string, model: string | null = null): AgentRun => ({
  kind: "task",
  conversationId: "c1",
  task,
  repo: "alpha",
  state: "ended",
  model,
  tokens: 1,
  createdAt: "2026-11-09T10:00:00Z",
  endedAt: null,
});

const many = (n: number) => Array.from({ length: n }, (_, i) => run(`task-${i}`));

describe("shownRuns", () => {
  it("keeps only the first few runs until the rest are asked for", () => {
    const runs = many(RUNS_SHOWN + 5);
    expect(shownRuns(runs, false)).toEqual(runs.slice(0, RUNS_SHOWN));
    expect(shownRuns(runs, true)).toEqual(runs);
  });

  it("shows every run when there are no more than the cap", () => {
    const runs = many(RUNS_SHOWN);
    expect(shownRuns(runs, false)).toEqual(runs);
  });

  it("caps at eight", () => {
    expect(RUNS_SHOWN).toBe(8);
  });
});

describe("runModel", () => {
  it("prefers the model the run recorded", () => {
    expect(runModel(run("a", "claude-haiku"), "sonnet")).toEqual({ name: "claude-haiku", inferred: false });
  });

  it("falls back to the agent type's model, marked as inferred", () => {
    expect(runModel(run("a"), "sonnet")).toEqual({ name: "sonnet", inferred: true });
  });

  it("is null when neither the run nor the type names a model", () => {
    expect(runModel(run("a"), null)).toBeNull();
  });
});

describe("showsModelColumn", () => {
  it("is off when no run has a model and the type has none", () => {
    expect(showsModelColumn([run("a"), run("b")], null)).toBe(false);
  });

  it("is on when any run recorded a model", () => {
    expect(showsModelColumn([run("a"), run("b", "claude-haiku")], null)).toBe(true);
  });

  it("is on when the type's model can fill the gaps", () => {
    expect(showsModelColumn([run("a")], "sonnet")).toBe(true);
  });
});
