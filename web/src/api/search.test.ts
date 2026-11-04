import { afterEach, describe, expect, it, vi } from "vitest";
import { ApiError } from "./errors";
import { parseSearchHit, parseSearchHits, searchConversations, type SearchHit } from "./search";

const titleHit: SearchHit = {
  conversationId: "c1",
  title: "login redirect loop",
  match: "title",
  entryId: null,
  snippet: null,
  updatedAt: "2026-11-04T10:42:05.000Z",
};

const messageHit: SearchHit = {
  conversationId: "c2",
  title: "shop work",
  match: "message",
  entryId: 7,
  snippet: "…look at the checkout flow before…",
  updatedAt: "2026-11-04T09:00:00.000Z",
};

function mockFetch(status: number, body: unknown) {
  const fn = vi.fn().mockResolvedValue(new Response(JSON.stringify(body), { status }));
  vi.stubGlobal("fetch", fn);
  return fn;
}

const ok = (data: unknown) => ({ success: true, data, error: null });

afterEach(() => vi.unstubAllGlobals());

describe("searchConversations", () => {
  it("GETs /api/search with the text encoded and returns the hits", async () => {
    const fn = mockFetch(200, ok([titleHit, messageHit]));
    expect(await searchConversations("100% & sø")).toEqual([titleHit, messageHit]);
    const [url, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(url).toBe("/api/search?q=100%25%20%26%20s%C3%B8");
    expect(init.method).toBe("GET");
    expect(init.body).toBeUndefined();
  });

  it("passes the abort signal on", async () => {
    const fn = mockFetch(200, ok([]));
    const controller = new AbortController();
    await searchConversations("login", controller.signal);
    const [, init] = fn.mock.calls[0] as [string, RequestInit];
    expect(init.signal).toBe(controller.signal);
  });

  it("answers a blank text with no hits and asks nothing", async () => {
    const fn = mockFetch(200, ok([titleHit]));
    expect(await searchConversations(" \n ")).toEqual([]);
    expect(fn).not.toHaveBeenCalled();
  });

  it("maps a 400 envelope to an http ApiError", async () => {
    mockFetch(400, { success: false, data: null, error: "search needs 1 to 200 characters" });
    const err = await searchConversations("x".repeat(201)).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(ApiError);
    expect(err).toMatchObject({ kind: "http", status: 400, message: "search needs 1 to 200 characters" });
  });

  it("rejects a malformed answer as a protocol error", async () => {
    mockFetch(200, ok({ hits: [] }));
    await expect(searchConversations("login")).rejects.toMatchObject({
      kind: "protocol",
      message: "unexpected response shape: search hits is not an array",
    });
  });
});

describe("parseSearchHit", () => {
  it("keeps the six fields, drops anything else and freezes the hit", () => {
    const parsed = parseSearchHit({ ...messageHit, score: 3 });
    expect(parsed).toEqual(messageHit);
    expect(Object.isFrozen(parsed)).toBe(true);
  });

  it.each([null, [], "hit", 3])("rejects %j as not an object", (value) => {
    expect(() => parseSearchHit(value)).toThrow("unexpected response shape: search hit is not an object");
  });

  it.each([
    ["conversationId", 1],
    ["title", null],
    ["match", "body"],
    ["updatedAt", undefined],
  ])("rejects an invalid %s", (key, value) => {
    expect(() => parseSearchHit({ ...titleHit, [key]: value })).toThrow(`searchHit.${key}`);
  });

  it.each([
    ["entryId", 2],
    ["snippet", "login"],
  ])("rejects a title hit with a %s", (key, value) => {
    expect(() => parseSearchHit({ ...titleHit, [key]: value })).toThrow(`searchHit.${key}`);
  });

  it.each([
    ["entryId", null],
    ["entryId", 1.5],
    ["entryId", "7"],
    ["snippet", null],
    ["snippet", 4],
  ])("rejects a message hit whose %s is %j", (key, value) => {
    expect(() => parseSearchHit({ ...messageHit, [key]: value })).toThrow(`searchHit.${key}`);
  });
});

describe("parseSearchHits", () => {
  it("parses every hit in order", () => {
    expect(parseSearchHits([messageHit, titleHit])).toEqual([messageHit, titleHit]);
  });

  it("names the broken hit's field", () => {
    expect(() => parseSearchHits([titleHit, { ...messageHit, entryId: null }])).toThrow(
      "unexpected response shape: searchHit.entryId",
    );
  });
});
