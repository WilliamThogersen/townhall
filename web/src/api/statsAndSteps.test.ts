import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aStep, someStats } from "./fixtures";
import { getStats } from "./stats";
import { getSteps } from "./steps";

afterEach(() => vi.unstubAllGlobals());

describe("stats api", () => {
  it("gets stats across repos or for one", async () => {
    const fn = mockFetch(200, someStats());
    expect(await getStats()).toEqual(someStats());
    await getStats("shop");
    expect(sent(fn, 0).url).toBe("/api/stats");
    expect(sent(fn, 1).url).toBe("/api/stats?repo=shop");
  });

  it("refuses a day without numbers", async () => {
    mockFetch(200, { days: [{ date: "2026-11-04", fixes: "2" }] });
    await expect(getStats()).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("steps api", () => {
  it("gets an agent's steps, with the repo when given", async () => {
    const steps = [aStep(), aStep({ id: "s2", endedAt: "2026-11-04T10:43:00.000Z", result: "ok" })];
    const fn = mockFetch(200, steps);
    expect(await getSteps("fix login")).toEqual(steps);
    await getSteps("fix-login", "shop");
    expect(sent(fn, 0).url).toBe("/api/agents/fix%20login/steps");
    expect(sent(fn, 1).url).toBe("/api/agents/fix-login/steps?repo=shop");
  });

  it("refuses an unknown step result", async () => {
    mockFetch(200, [{ ...aStep(), result: "maybe" }]);
    await expect(getSteps("a")).rejects.toMatchObject({ kind: "protocol" });
  });
});
