import { afterEach, describe, expect, it, vi } from "vitest";
import { mockFetch, sent } from "../test/fetchMock";
import { getDetectedAccounts } from "./forgeAccounts";

afterEach(() => vi.unstubAllGlobals());

const found = { forge: "github", login: "work-me", host: "github.com", registered: false };

describe("forge accounts api", () => {
  it("asks the daemon which accounts gh and glab are logged in with", async () => {
    const fn = mockFetch(200, [found, { ...found, forge: "gitlab", login: "me", registered: true }]);
    expect(await getDetectedAccounts()).toEqual({
      accounts: [found, { forge: "gitlab", login: "me", host: "github.com", registered: true }],
      warnings: [],
    });
    expect(sent(fn)).toEqual({ url: "/api/forge-accounts/detected", method: "GET", body: undefined });
  });

  it("accepts an empty list", async () => {
    mockFetch(200, []);
    expect(await getDetectedAccounts()).toEqual({ accounts: [], warnings: [] });
  });

  it("reads the optional warnings beside data", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => Response.json({ success: true, data: [], error: null, warnings: ["glab: no glab config", 3] })),
    );
    expect((await getDetectedAccounts()).warnings).toEqual(["glab: no glab config"]);
  });

  it("refuses a shape it does not know, and an unknown forge", async () => {
    mockFetch(200, [{ ...found, forge: "bitbucket" }]);
    await expect(getDetectedAccounts()).rejects.toThrow("unexpected response shape");
    mockFetch(200, [{ forge: "github", login: "x", host: "github.com" }]);
    await expect(getDetectedAccounts()).rejects.toThrow("unexpected response shape");
    mockFetch(200, { accounts: [] });
    await expect(getDetectedAccounts()).rejects.toThrow("unexpected response shape");
  });
});
