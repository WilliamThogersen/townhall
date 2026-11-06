import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "./errors";
import { deleteProfile, getProfile, parseProfile, putProfile } from "./profile";

function mockFetch(status: number, body: unknown) {
  const fn = vi.fn().mockResolvedValue(
    new Response(typeof body === "string" ? body : JSON.stringify(body), { status }),
  );
  vi.stubGlobal("fetch", fn);
  return fn;
}

const ok = (data: unknown) => ({ success: true, data, error: null });

afterEach(() => vi.unstubAllGlobals());

describe("getProfile", () => {
  it("GETs /api/profile and unwraps the envelope", async () => {
    const fn = mockFetch(200, ok({ text: "terse, tests first" }));
    expect(await getProfile()).toEqual({ text: "terse, tests first" });
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/profile");
    expect(init.method).toBe("GET");
    expect(init.body).toBeUndefined();
  });

  it("passes the abort signal on", async () => {
    const fn = mockFetch(200, ok({ text: "" }));
    const controller = new AbortController();
    await getProfile(controller.signal);
    const [, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(init.signal).toBe(controller.signal);
  });

  it("maps a 503 envelope to an http ApiError", async () => {
    mockFetch(503, { success: false, data: null, error: "townhall.db is held by another townhall" });
    const err = await getProfile().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ kind: "http", status: 503, message: "townhall.db is held by another townhall" });
  });

  it("maps fetch failures to network errors", async () => {
    vi.stubGlobal("fetch", vi.fn().mockRejectedValue(new TypeError("boom")));
    await expect(getProfile()).rejects.toMatchObject({ kind: "network" });
  });
});

describe("putProfile", () => {
  it("PUTs { text } to /api/profile and returns the stored text", async () => {
    const fn = mockFetch(200, ok({ text: "new text" }));
    expect(await putProfile("new text")).toEqual({ text: "new text" });
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/profile");
    expect(init.method).toBe("PUT");
    expect(init.headers).toEqual({
      "content-type": "application/json",
      "x-request-id": expect.any(String),
    });
    expect(JSON.parse(init.body as string)).toEqual({ text: "new text" });
  });

  it("maps a 400 for too long a profile to an http ApiError", async () => {
    mockFetch(400, { success: false, data: null, error: "profile is too long" });
    await expect(putProfile("x")).rejects.toMatchObject({
      kind: "http",
      status: 400,
      message: "profile is too long",
    });
  });
});

describe("deleteProfile", () => {
  it("DELETEs /api/profile without a body and returns an empty text", async () => {
    const fn = mockFetch(200, ok({ text: "" }));
    expect(await deleteProfile()).toEqual({ text: "" });
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/profile");
    expect(init.method).toBe("DELETE");
    expect(init.body).toBeUndefined();
  });
});

describe("parseProfile", () => {
  it("keeps text and drops anything else", () => {
    const parsed = parseProfile({ text: "hi", extra: 1 });
    expect(parsed).toEqual({ text: "hi" });
    expect(Object.isFrozen(parsed)).toBe(true);
  });

  it.each([null, [], "profile", 3])("rejects %j as not an object", (value) => {
    expect(() => parseProfile(value)).toThrow("unexpected response shape: profile is not an object");
  });

  it.each([{}, { text: 3 }])("rejects %j without a string text", (value) => {
    expect(() => parseProfile(value)).toThrow("unexpected response shape: profile.text");
  });
});
