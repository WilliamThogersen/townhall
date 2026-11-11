import { afterEach, describe, expect, it, vi } from "vitest";
import { aRun, aRunStep } from "./fixtures";
import { getRun, listRuns, parseRun, parseRuns } from "./runs";

afterEach(() => vi.unstubAllGlobals());

function stubFetch(data: unknown): ReturnType<typeof vi.fn> {
  const fetchMock = vi
    .fn()
    .mockResolvedValue(new Response(JSON.stringify({ success: true, data, error: null }), { status: 200 }));
  vi.stubGlobal("fetch", fetchMock);
  return fetchMock;
}

describe("parseRun", () => {
  it("accepts what the daemon sends", () => {
    const run = aRun({ steps: [aRunStep({ id: "plan", state: "done", round: 1, task: "fix-login-plan" })] });
    expect(parseRun(run)).toEqual(run);
    expect(parseRuns([run])).toEqual([run]);
  });

  it("refuses a state it does not know", () => {
    expect(() => parseRun({ ...aRun(), state: "paused" })).toThrow("run.state");
    expect(() => parseRun({ ...aRun(), steps: [{ ...aRunStep(), state: "lost" }] })).toThrow("run step.state");
    expect(() => parseRuns({})).toThrow("runs is not an array");
  });
});

describe("runs requests", () => {
  it("lists the runs of a conversation", async () => {
    const fetchMock = stubFetch([aRun()]);
    expect(await listRuns("c 1")).toEqual([aRun()]);
    expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/runs?conversation=c+1");
  });

  it("reads one run", async () => {
    const fetchMock = stubFetch(aRun());
    expect(await getRun("fix-login")).toEqual(aRun());
    expect(fetchMock.mock.calls[0]?.[0]).toBe("/api/runs/fix-login");
  });
});
