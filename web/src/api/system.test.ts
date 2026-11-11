import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { aSystem } from "./fixtures";
import { getSystem } from "./system";

afterEach(() => vi.unstubAllGlobals());

describe("system api", () => {
  it("gets the machine stats", async () => {
    const fn = mockFetch(200, aSystem());
    expect(await getSystem()).toEqual(aSystem());
    expect(sent(fn).url).toBe("/api/system");
  });

  it("refuses a reading with a missing or non numeric field", async () => {
    mockFetch(200, { ...aSystem(), cpuPercent: "high" });
    await expect(getSystem()).rejects.toMatchObject({
      kind: "protocol",
      message: expect.stringContaining("system.cpuPercent"),
    });
    const { diskTotalBytes: _gone, ...partial } = aSystem();
    mockFetch(200, partial);
    await expect(getSystem()).rejects.toMatchObject({ kind: "protocol" });
  });
});
