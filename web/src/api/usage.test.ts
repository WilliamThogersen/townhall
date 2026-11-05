import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aUsage } from "./fixtures";
import { getUsage } from "./usage";

afterEach(() => vi.unstubAllGlobals());

describe("usage api", () => {
  it("gets the plan usage", async () => {
    const fn = mockFetch(200, aUsage());
    expect(await getUsage()).toEqual(aUsage());
    expect(sent(fn).url).toBe("/api/usage");
  });

  it("takes empty windows and an error as they come", async () => {
    const empty = { session: null, weekly: null, updatedAt: 5, error: "Sign in to Claude Code to see usage" };
    mockFetch(200, empty);
    expect(await getUsage()).toEqual(empty);
  });

  it("reads missing optional fields as null", async () => {
    mockFetch(200, { session: { usedPercent: 12 }, updatedAt: 5 });
    expect(await getUsage()).toEqual({
      session: { usedPercent: 12, resetsAt: null, ratePerHour: null },
      weekly: null,
      updatedAt: 5,
      error: null,
    });
  });

  it("refuses a window without a percentage and a reading without a time", async () => {
    mockFetch(200, aUsage({ session: { usedPercent: "high" } as never }));
    await expect(getUsage()).rejects.toMatchObject({ kind: "protocol", message: expect.stringContaining("usage.session.usedPercent") });
    mockFetch(200, { session: null, weekly: null, error: null });
    await expect(getUsage()).rejects.toMatchObject({ kind: "protocol" });
  });
});
