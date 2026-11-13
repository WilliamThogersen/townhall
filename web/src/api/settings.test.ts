import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "./errors";
import { getSettings, parseSettingsData, putSetting } from "./settings";
import type { Settings } from "./types";

const daemon: Settings = {
  agentsAtOnce: 9,
  repoUnclear: "guess",
  sessionPanelOpen: false,
  notifyWhen: "failed",
  minFreeDiskGb: 10,
  learnAboutMe: true,
  githubAccount: "octocat",
  forgeAccounts: [{ forge: "gitlab", login: "me", host: "gitlab.corp.io", label: "work" }],
  reflectAfterTasks: true,
  clonesDir: "~/Developer",
  flakyChecks: ["lint"],
  adviserPermissions: false,
  permissionTimeoutSecs: 90,
};

function mockFetch(status: number, body: unknown) {
  const fn = vi.fn().mockResolvedValue(
    new Response(typeof body === "string" ? body : JSON.stringify(body), { status }),
  );
  vi.stubGlobal("fetch", fn);
  return fn;
}

const ok = (data: unknown) => ({ success: true, data, error: null });

afterEach(() => vi.unstubAllGlobals());

describe("getSettings", () => {
  it("GETs /api/settings and unwraps the envelope", async () => {
    const fn = mockFetch(200, ok(daemon));
    expect(await getSettings()).toEqual(daemon);
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/settings");
    expect(init.method).toBe("GET");
    expect(init.body).toBeUndefined();
  });

  it("passes the abort signal on", async () => {
    const fn = mockFetch(200, ok(daemon));
    const controller = new AbortController();
    await getSettings(controller.signal);
    const [, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(init.signal).toBe(controller.signal);
  });

  it("maps a 503 envelope to an http ApiError", async () => {
    mockFetch(503, { success: false, data: null, error: "townhall.db is held by another townhall" });
    const err = await getSettings().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ kind: "http", status: 503, message: "townhall.db is held by another townhall" });
  });

  it("maps fetch failures to network errors", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("boom")));
    await expect(getSettings()).rejects.toMatchObject({ kind: "network" });
  });

  it("rejects settings with a missing or invalid field", async () => {
    mockFetch(200, ok({ ...daemon, agentsAtOnce: 99 }));
    await expect(getSettings()).rejects.toMatchObject({
      kind: "protocol",
      message: "unexpected response shape: settings.agentsAtOnce",
    });
  });
});

describe("putSetting", () => {
  it("PUTs the value as { value } to /api/settings/{key}", async () => {
    const after = { ...daemon, agentsAtOnce: 12 };
    const fn = mockFetch(200, ok(after));
    expect(await putSetting("agentsAtOnce", 12)).toEqual(after);
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/settings/agentsAtOnce");
    expect(init.method).toBe("PUT");
    expect(init.headers).toEqual({
      "content-type": "application/json",
      "x-request-id": expect.any(String),
    });
    expect(JSON.parse(init.body as string)).toEqual({ value: 12 });
  });

  it("sends the whole account list for forgeAccounts", async () => {
    const accounts = [{ forge: "github", login: "work-me", host: "github.com", label: "work" }] as const;
    const fn = mockFetch(200, ok({ ...daemon, forgeAccounts: accounts }));
    expect((await putSetting("forgeAccounts", accounts)).forgeAccounts).toEqual(accounts);
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/settings/forgeAccounts");
    expect(JSON.parse(init.body as string)).toEqual({ value: accounts });
  });

  it("maps a 400 for an invalid value to an http ApiError", async () => {
    mockFetch(400, { success: false, data: null, error: "agentsAtOnce must be 1 to 50" });
    await expect(putSetting("agentsAtOnce", 6)).rejects.toMatchObject({
      kind: "http",
      status: 400,
      message: "agentsAtOnce must be 1 to 50",
    });
  });

  it("rejects a malformed answer as a protocol error", async () => {
    mockFetch(200, ok([daemon]));
    await expect(putSetting("repoUnclear", "ask")).rejects.toMatchObject({ kind: "protocol" });
  });
});

describe("parseSettingsData", () => {
  it("keeps the known settings and drops anything else, the removed permission ones included", () => {
    const parsed = parseSettingsData({
      ...daemon,
      theme: "light",
      permissionMode: "bypassPermissions",
      workWithoutAsking: true,
    });
    expect(parsed).toEqual(daemon);
    expect(Object.isFrozen(parsed)).toBe(true);
  });

  it.each([null, [], "settings", 3])("rejects %j as not an object", (value) => {
    expect(() => parseSettingsData(value)).toThrow("unexpected response shape: settings is not an object");
  });

  it.each([
    ["agentsAtOnce", 2.5],
    ["repoUnclear", undefined],
    ["sessionPanelOpen", null],
    ["notifyWhen", "never"],
    ["forgeAccounts", [{ forge: "github", login: "me" }]],
    ["forgeAccounts", undefined],
  ])("rejects an invalid %s", (key, value) => {
    expect(() => parseSettingsData({ ...daemon, [key]: value })).toThrow(`settings.${key}`);
  });
});
