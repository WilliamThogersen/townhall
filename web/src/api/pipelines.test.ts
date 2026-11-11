import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aPipeline } from "./fixtures";
import { definitionErrors, deletePipeline, getPipeline, listPipelines, savePipeline } from "./pipelines";

afterEach(() => vi.unstubAllGlobals());

const wire = (over: Record<string, unknown> = {}) => ({ ...aPipeline(), origin: "user", plugin: null, ...over });

describe("pipelines api", () => {
  it("lists pipelines", async () => {
    const fn = mockFetch(200, [wire()]);
    const found = await listPipelines();
    expect(found[0]).toMatchObject({ name: "standard", steps: [{ id: "plan", agent: "planner" }, { id: "build", agent: "builder" }] });
    expect(sent(fn)).toEqual({ url: "/api/library/workflows", method: "GET", body: undefined });
  });

  it("reads one pipeline with its TOML source", async () => {
    const fn = mockFetch(200, wire({ name: "my flow" }));
    const found = await getPipeline("my flow");
    expect(found.source).toContain("standard");
    expect(sent(fn).url).toBe("/api/library/workflows/my%20flow");
  });

  it("reads where a pipeline comes from", async () => {
    mockFetch(200, wire({ origin: "plugin", plugin: "flows@acme" }));
    expect(await getPipeline("flows-ship")).toMatchObject({ origin: "plugin", plugin: "flows@acme" });
    mockFetch(200, wire({ origin: "robot" }));
    await expect(getPipeline("x")).rejects.toMatchObject({ kind: "protocol" });
  });

  it("saves the TOML as the body of a PUT", async () => {
    const fn = mockFetch(200, wire());
    await savePipeline("standard", 'name = "standard"\n');
    const [url, init] = fn.mock.calls[0] ?? [];
    expect(url).toBe("/api/library/workflows/standard");
    expect(init?.method).toBe("PUT");
    expect(init?.body).toBe('name = "standard"\n');
    expect(new Headers(init?.headers).get("content-type")).toBe("application/toml");
  });

  it("deletes by name", async () => {
    const fn = mockFetch(200, "standard");
    expect(await deletePipeline("standard")).toBe("standard");
    expect(sent(fn)).toEqual({ url: "/api/library/workflows/standard", method: "DELETE", body: undefined });
  });

  it("carries the field errors of a refused save", async () => {
    const errors = [
      { kind: "workflow", name: "standard", step: "build", field: "after", message: "cycle through plan", line: null },
      { kind: "workflow", name: "standard", step: null, field: "work", message: 'no step "ship"', line: 7 },
    ];
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json({ success: false, data: errors, error: "refused" }, { status: 400 })),
    );
    const failure: unknown = await savePipeline("standard", "x").catch((e: unknown) => e);
    expect(failure).toMatchObject({ message: "refused", status: 400 });
    expect(definitionErrors(failure)).toEqual(errors);
  });

  it("has no field errors for a failure that carries none", async () => {
    expect(definitionErrors(new Error("boom"))).toEqual([]);
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json({ success: false, data: null, error: "gone" }, { status: 404 })),
    );
    const failure: unknown = await getPipeline("x").catch((e: unknown) => e);
    expect(definitionErrors(failure)).toEqual([]);
  });
});
