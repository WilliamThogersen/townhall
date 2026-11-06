import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "./errors";
import { getSettings, parseSettingsData, putSetting } from "./settings";
import type { Settings } from "./types";

const daemon: Settings = {
  permissionMode: "plan",
  workWithoutAsking: true,
  agentsAtOnce: 9,
  repoUnclear: "guess",
  sessionPanelOpen: false,
  notifyWhen: "failed",
  townhallModel: "haiku",
  learnAboutMe: true,
  githubAccount: "octocat",
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

  it("sends skip all as bypassPermissions", async () => {
    const fn = mockFetch(200, ok({ ...daemon, permissionMode: "bypassPermissions" }));
    await putSetting("permissionMode", "bypassPermissions");
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/settings/permissionMode");
    expect(JSON.parse(init.body as string)).toEqual({ value: "bypassPermissions" });
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
  it("keeps the six settings and drops anything else", () => {
    const parsed = parseSettingsData({ ...daemon, theme: "light" });
    expect(parsed).toEqual(daemon);
    expect(Object.isFrozen(parsed)).toBe(true);
  });

  it.each([null, [], "settings", 3])("rejects %j as not an object", (value) => {
    expect(() => parseSettingsData(value)).toThrow("unexpected response shape: settings is not an object");
  });

  it.each([
    ["permissionMode", "yolo"],
    ["workWithoutAsking", "yes"],
    ["agentsAtOnce", 2.5],
    ["repoUnclear", undefined],
    ["sessionPanelOpen", null],
    ["notifyWhen", "never"],
  ])("rejects an invalid %s", (key, value) => {
    expect(() => parseSettingsData({ ...daemon, [key]: value })).toThrow(`settings.${key}`);
  });
});
