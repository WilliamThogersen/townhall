import { ApiError } from "./errors";
import { request } from "./request";

interface SearchHitBase {
  readonly conversationId: string;
  readonly title: string;
  readonly updatedAt: string;
}

export interface TitleHit extends SearchHitBase {
  readonly match: "title";
  readonly entryId: null;
  readonly snippet: null;
}

export interface MessageHit extends SearchHitBase {
  readonly match: "message";
  readonly entryId: number;
  readonly snippet: string;
}

export type SearchHit = TitleHit | MessageHit;

export type SearchMatch = SearchHit["match"];

export const SEARCH_MAX_CHARS = 200;

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function fail(what: string): never {
  throw new ApiError("protocol", `unexpected response shape: ${what}`);
}

function parseBase(value: Record<string, unknown>): SearchHitBase {
  const { conversationId, title, updatedAt } = value;
  if (typeof conversationId !== "string") return fail("searchHit.conversationId");
  if (typeof title !== "string") return fail("searchHit.title");
  if (typeof updatedAt !== "string") return fail("searchHit.updatedAt");
  return { conversationId, title, updatedAt };
}

function parseTitleHit(base: SearchHitBase, value: Record<string, unknown>): TitleHit {
  if (value.entryId !== null) return fail("searchHit.entryId");
  if (value.snippet !== null) return fail("searchHit.snippet");
  return Object.freeze({ ...base, match: "title", entryId: null, snippet: null });
}

function parseMessageHit(base: SearchHitBase, value: Record<string, unknown>): MessageHit {
  const { entryId, snippet } = value;
  if (typeof entryId !== "number" || !Number.isInteger(entryId)) return fail("searchHit.entryId");
  if (typeof snippet !== "string") return fail("searchHit.snippet");
  return Object.freeze({ ...base, match: "message", entryId, snippet });
}

export function parseSearchHit(value: unknown): SearchHit {
  if (!isRecord(value)) return fail("search hit is not an object");
  const base = parseBase(value);
  if (value.match === "title") return parseTitleHit(base, value);
  if (value.match === "message") return parseMessageHit(base, value);
  return fail("searchHit.match");
}

export function parseSearchHits(value: unknown): readonly SearchHit[] {
  if (!Array.isArray(value)) return fail("search hits is not an array");
  return Object.freeze(value.map(parseSearchHit));
}

export function searchConversations(q: string, signal?: AbortSignal): Promise<readonly SearchHit[]> {
  if (q.trim() === "") return Promise.resolve([]);
  return request("GET", `/search?q=${encodeURIComponent(q)}`, parseSearchHits, undefined, signal);
}
