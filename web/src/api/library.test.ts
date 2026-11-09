import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { anAgentType, anAgentTypeSummary } from "./fixtures";
import { getAgentType, listAgentRuns, listAgentTypes, listAgentUsage } from "./library";

afterEach(() => vi.unstubAllGlobals());

describe("library api", () => {
  it("lists agent types", async () => {
    const fn = mockFetch(200, [anAgentTypeSummary()]);
    expect(await listAgentTypes()).toEqual([anAgentTypeSummary()]);
    expect(sent(fn)).toEqual({ url: "/api/library/agents", method: "GET", body: undefined });
  });

  it("reads one agent type with its TOML source", async () => {
    const fn = mockFetch(200, anAgentType({ name: "my type", tools: ["Read"] }));
    const type = await getAgentType("my type");
    expect(type.tools).toEqual(["Read"]);
    expect(type.source).toContain("builder");
    expect(sent(fn).url).toBe("/api/library/agents/my%20type");
  });

  it("reads which types an agent type may hand back to", async () => {
    mockFetch(200, anAgentType({ canHandBackTo: ["coder"] }));
    expect((await getAgentType("builder")).canHandBackTo).toEqual(["coder"]);
    mockFetch(200, anAgentType({ canHandBackTo: null }));
    expect((await getAgentType("builder")).canHandBackTo).toBeNull();
    mockFetch(200, { ...anAgentType(), canHandBackTo: "coder" });
    await expect(getAgentType("builder")).rejects.toMatchObject({ kind: "protocol" });
  });

  it("reads the env map of a type, empty by default", async () => {
    mockFetch(200, anAgentType());
    expect((await getAgentType("builder")).env).toEqual({});
    mockFetch(200, anAgentType({ env: { TUNE_ME: "5" } }));
    expect((await getAgentType("builder")).env).toEqual({ TUNE_ME: "5" });
    mockFetch(200, { ...anAgentType(), env: { TUNE_ME: 5 } });
    await expect(getAgentType("builder")).rejects.toMatchObject({ kind: "protocol" });
    mockFetch(200, { ...anAgentType(), env: ["TUNE_ME"] });
    await expect(getAgentType("builder")).rejects.toMatchObject({ kind: "protocol" });
  });

  it("accepts null model and permission mode", async () => {
    mockFetch(200, [anAgentTypeSummary({ model: null, permissionMode: null })]);
    expect(await listAgentTypes()).toHaveLength(1);
  });

  it("refuses an unknown adapter", async () => {
    mockFetch(200, [{ ...anAgentTypeSummary(), adapter: "gpt" }]);
    await expect(listAgentTypes()).rejects.toMatchObject({ kind: "protocol" });
  });

  it("refuses a type without source", async () => {
    mockFetch(200, { ...anAgentType(), source: undefined });
    await expect(getAgentType("builder")).rejects.toMatchObject({ kind: "protocol" });
  });

  it("lists runs and tokens per agent type", async () => {
    const fn = mockFetch(200, [{ agent: "builder", runs: 2, tokens: 1500 }]);
    expect(await listAgentUsage()).toEqual([{ agent: "builder", runs: 2, tokens: 1500 }]);
    expect(sent(fn)).toEqual({ url: "/api/library/agents/usage", method: "GET", body: undefined });
  });

  it("refuses usage without a token count", async () => {
    mockFetch(200, [{ agent: "builder", runs: 2 }]);
    await expect(listAgentUsage()).rejects.toMatchObject({ kind: "protocol" });
  });

  it("lists the recent runs of an agent type", async () => {
    const run = { task: "one", repo: "alpha", state: "ended", model: null, tokens: 12, createdAt: "2026-11-09T10:00:00Z", endedAt: null };
    const fn = mockFetch(200, [run, { ...run, task: "two", model: "claude-x", endedAt: "2026-11-09T11:00:00Z" }]);
    const runs = await listAgentRuns("my type");
    expect(runs.map((r) => [r.task, r.model, r.endedAt])).toEqual([
      ["one", null, null],
      ["two", "claude-x", "2026-11-09T11:00:00Z"],
    ]);
    expect(sent(fn)).toEqual({ url: "/api/library/agents/my%20type/runs", method: "GET", body: undefined });
  });

  it("refuses a run without a token count", async () => {
    mockFetch(200, [{ task: "one", repo: "alpha", state: "ended", model: null, createdAt: "x", endedAt: null }]);
    await expect(listAgentRuns("builder")).rejects.toMatchObject({ kind: "protocol" });
  });
});
