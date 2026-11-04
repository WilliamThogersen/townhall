import type { Conversation, Task } from "../api/types";
import { groupByRepo, inferRepo, repoOfConversation, type ConversationRepo } from "./conversationRepo";
import { summarizeRow, type RowSummary } from "./rowSummary";
import { foldParts, splitSections, type RowFilter } from "./sections";

export interface SidebarRow {
  readonly conversation: Conversation;
  readonly summary: RowSummary;
  readonly repo: ConversationRepo;
  readonly autoRepo: ConversationRepo;
}

export interface RowSections {
  readonly active: readonly SidebarRow[];
  readonly older: readonly SidebarRow[];
}

export interface GroupBlock {
  readonly key: string;
  readonly label: string;
  readonly count: number;
  readonly open: boolean;
  readonly rows: readonly SidebarRow[];
}

export interface FoldState {
  readonly folded: ReadonlySet<string>;
  readonly currentId: string | null;
  readonly searching: boolean;
}

const NO_TASKS: readonly Task[] = [];

function tasksByConversation(tasks: readonly Task[]): ReadonlyMap<string, readonly Task[]> {
  return tasks.reduce(
    (all, task) => new Map(all).set(task.conversationId, [...(all.get(task.conversationId) ?? []), task]),
    new Map<string, readonly Task[]>(),
  );
}

export function buildSections(
  conversations: readonly Conversation[],
  tasks: readonly Task[],
  filter?: RowFilter,
): RowSections {
  const owned = tasksByConversation(tasks);
  const makeRow = (conversation: Conversation): SidebarRow => {
    const own = owned.get(conversation.id) ?? NO_TASKS;
    return {
      conversation,
      summary: summarizeRow(conversation, own),
      repo: repoOfConversation(conversation, own),
      autoRepo: inferRepo(own),
    };
  };
  const rows = new Map(conversations.map((conversation) => [conversation.id, makeRow(conversation)] as const));
  const rowOf = (conversation: Conversation) => rows.get(conversation.id) ?? makeRow(conversation);
  const { active, older } = splitSections(conversations, (c) => rowOf(c).summary.needsYou, filter);
  return { active: active.map(rowOf), older: older.map(rowOf) };
}

function shownRows(rows: readonly SidebarRow[], open: boolean, currentId: string | null): readonly SidebarRow[] {
  if (open) return rows;
  const { current } = foldParts(
    rows.map((row) => row.conversation),
    currentId,
  );
  return rows.filter((row) => row.conversation === current);
}

export function repoBlocks(active: readonly SidebarRow[], fold: FoldState): readonly GroupBlock[] {
  return groupByRepo(active, (row) => row.repo).map((group) => {
    const open = fold.searching || !fold.folded.has(group.key);
    return {
      key: group.key,
      label: group.label,
      count: group.items.length,
      open,
      rows: shownRows(group.items, open, fold.currentId),
    };
  });
}

export function visibleIds(
  sections: RowSections,
  blocks: readonly GroupBlock[] | null,
): readonly string[] {
  const active = blocks ? blocks.flatMap((block) => block.rows) : sections.active;
  return [...active, ...sections.older].map((row) => row.conversation.id);
}
