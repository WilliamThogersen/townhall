import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { anAgentType, anAgentTypeSummary } from "./fixtures";
import { getAgentType, listAgentTypes, listAgentUsage } from "./library";

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
});
