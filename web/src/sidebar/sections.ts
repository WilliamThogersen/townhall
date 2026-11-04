import type { Conversation } from "../api/types";

export interface Sections {
  readonly active: readonly Conversation[];
  readonly older: readonly Conversation[];
}

export interface RowFilter {
  readonly query: string;
  readonly textMatches: ReadonlySet<string>;
}

export interface FoldParts {
  readonly before: readonly Conversation[];
  readonly current: Conversation | null;
  readonly after: readonly Conversation[];
}

export const normalizeQuery = (text: string): string => text.trim().split(/\s+/).join(" ").toLowerCase();

export const matchesTitle = (title: string, normalized: string): boolean =>
  normalizeQuery(title).includes(normalized);

const newestFirst = (a: Conversation, b: Conversation) =>
  b.updatedAt.localeCompare(a.updatedAt) || b.createdAt.localeCompare(a.createdAt);

function shownBy(filter: RowFilter | undefined): (conversation: Conversation) => boolean {
  const normalized = filter ? normalizeQuery(filter.query) : "";
  if (!filter || normalized === "") return () => true;
  return (conversation) =>
    matchesTitle(conversation.title, normalized) || filter.textMatches.has(conversation.id);
}

export function splitSections(
  conversations: readonly Conversation[],
  needsYou: (conversation: Conversation) => boolean,
  filter?: RowFilter,
): Sections {
  const shown = conversations.filter(shownBy(filter));
  const attentionFirst = (a: Conversation, b: Conversation) =>
    Number(needsYou(b)) - Number(needsYou(a)) || newestFirst(a, b);
  return {
    active: shown.filter((c) => !c.ended).sort(attentionFirst),
    older: shown.filter((c) => c.ended).sort(newestFirst),
  };
}

export function foldParts(rows: readonly Conversation[], currentId: string | null): FoldParts {
  const at = currentId === null ? -1 : rows.findIndex((row) => row.id === currentId);
  if (at < 0) return { before: rows, current: null, after: [] };
  return { before: rows.slice(0, at), current: rows[at] ?? null, after: rows.slice(at + 1) };
}
