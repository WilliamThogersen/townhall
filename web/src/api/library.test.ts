import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { anAgentType, anAgentTypeSummary } from "./fixtures";
import {
  getAgentType,
  getAgentUsageSeries,
  listAgentRuns,
  listAgentTypes,
  listAgentUsage,
  saveAgentType,
} from "./library";

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

  it("reads effort on the list and the detail, null when unset, and refuses an unknown level", async () => {
    mockFetch(200, [anAgentTypeSummary({ effort: "high" })]);
    expect((await listAgentTypes())[0]?.effort).toBe("high");
    mockFetch(200, anAgentType({ effort: "max" }));
    expect((await getAgentType("builder")).effort).toBe("max");
    mockFetch(200, anAgentType());
    expect((await getAgentType("builder")).effort).toBeNull();
    mockFetch(200, [{ ...anAgentTypeSummary(), effort: "extreme" }]);
    await expect(listAgentTypes()).rejects.toMatchObject({ kind: "protocol" });
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
    const run = { kind: "task", conversationId: "c1", task: "one", repo: "alpha", state: "ended", model: null, tokens: 12, createdAt: "2026-11-09T10:00:00Z", endedAt: null };
    const fn = mockFetch(200, [run, { ...run, kind: "turn", task: "two", model: "claude-x", endedAt: "2026-11-09T11:00:00Z" }]);
    const runs = await listAgentRuns("my type");
    expect(runs.map((r) => [r.kind, r.conversationId, r.task, r.model, r.endedAt])).toEqual([
      ["task", "c1", "one", null, null],
      ["turn", "c1", "two", "claude-x", "2026-11-09T11:00:00Z"],
    ]);
    expect(sent(fn)).toEqual({ url: "/api/library/agents/my%20type/runs", method: "GET", body: undefined });
  });

  it("refuses a run without a token count", async () => {
    mockFetch(200, [{ kind: "task", conversationId: "c1", task: "one", repo: "alpha", state: "ended", model: null, createdAt: "x", endedAt: null }]);
    await expect(listAgentRuns("builder")).rejects.toMatchObject({ kind: "protocol" });
  });

  it("refuses a run of an unknown kind or without a conversation", async () => {
    const run = { kind: "task", conversationId: "c1", task: "one", repo: "alpha", state: "ended", model: null, tokens: 1, createdAt: "x", endedAt: null };
    mockFetch(200, [{ ...run, kind: "workflow" }]);
    await expect(listAgentRuns("builder")).rejects.toMatchObject({ kind: "protocol" });
    mockFetch(200, [{ ...run, conversationId: undefined }]);
    await expect(listAgentRuns("builder")).rejects.toMatchObject({ kind: "protocol" });
  });
});

const wireAvatar = (over: Record<string, unknown> = {}) => ({
  head: null,
  eyes: null,
  mouth: null,
  accessory: null,
  color: null,
  grid: null,
  ...over,
});

describe("library api avatar", () => {
  it("reads no avatar as null", async () => {
    mockFetch(200, [{ ...anAgentTypeSummary(), avatar: null }]);
    expect((await listAgentTypes())[0]?.avatar).toBeNull();
  });

  it("reads the keys that are set and drops the null ones", async () => {
    mockFetch(200, [{ ...anAgentTypeSummary(), avatar: wireAvatar({ head: "round", color: "fg-muted" }) }]);
    expect((await listAgentTypes())[0]?.avatar).toEqual({ head: "round", color: "fg-muted" });
  });

  it("reads a grid", async () => {
    const grid = Array.from({ length: 9 }, () => "#########");
    mockFetch(200, { ...anAgentType(), avatar: wireAvatar({ grid }) });
    expect((await getAgentType("builder")).avatar).toEqual({ grid });
  });

  it("refuses an avatar value the app does not know", async () => {
    mockFetch(200, [{ ...anAgentTypeSummary(), avatar: wireAvatar({ head: "cube" }) }]);
    await expect(listAgentTypes()).rejects.toMatchObject({ kind: "protocol" });
    mockFetch(200, { ...anAgentType(), avatar: wireAvatar({ grid: "#########" }) });
    await expect(getAgentType("builder")).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("saveAgentType", () => {
  it("puts the TOML text as application/toml and reads the saved type back", async () => {
    const source = 'name = "my type"\nadapter = "claude"\n';
    const fn = mockFetch(200, anAgentType({ name: "my type", source }));
    const saved = await saveAgentType("my type", source);
    expect(saved.source).toBe(source);
    const [url, init] = fn.mock.calls[0] ?? [];
    expect(url).toBe("/api/library/agents/my%20type");
    expect(init?.method).toBe("PUT");
    expect(init?.body).toBe(source);
    expect(new Headers(init?.headers).get("content-type")).toBe("application/toml");
  });

  it("surfaces the daemon's list of definition errors", async () => {
    mockFetch(400, null, 'avatar.grid must have 9 rows, found 1');
    await expect(saveAgentType("builder", "x")).rejects.toMatchObject({
      kind: "http",
      status: 400,
      message: "avatar.grid must have 9 rows, found 1",
    });
  });
});

const wireSeries = (over: Record<string, unknown> = {}) => ({
  agent: "builder",
  range: "24h",
  bucketMs: 3_600_000,
  since: "2026-11-04T00:00:00.000Z",
  until: "2026-11-05T00:00:00.000Z",
  runs: 2,
  tokens: 150,
  buckets: [
    { start: "2026-11-04T00:00:00.000Z", runs: 1, tokens: 100 },
    { start: "2026-11-04T01:00:00.000Z", runs: 1, tokens: 50 },
  ],
  byTask: [{ key: "a", runs: 1, tokens: 100 }],
  byRepo: [{ key: "shop", runs: 2, tokens: 150 }],
  byConversation: [{ key: "c1", runs: 2, tokens: 150 }],
  ...over,
});

describe("getAgentUsageSeries", () => {
  it("asks for one type and range and reads the series", async () => {
    const fn = mockFetch(200, wireSeries({ range: "7d" }));
    const series = await getAgentUsageSeries("my type", "7d");
    expect(sent(fn)).toEqual({ url: "/api/library/agents/my%20type/usage?range=7d", method: "GET", body: undefined });
    expect(series.range).toBe("7d");
    expect(series.buckets).toHaveLength(2);
    expect(series.byRepo).toEqual([{ key: "shop", runs: 2, tokens: 150 }]);
  });

  it("refuses a range the app does not know and a bucket without tokens", async () => {
    mockFetch(200, wireSeries({ range: "1y" }));
    await expect(getAgentUsageSeries("builder", "24h")).rejects.toMatchObject({ kind: "protocol" });
    mockFetch(200, wireSeries({ buckets: [{ start: "x", runs: 1 }] }));
    await expect(getAgentUsageSeries("builder", "24h")).rejects.toMatchObject({ kind: "protocol" });
  });
});
