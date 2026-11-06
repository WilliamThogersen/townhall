import type { AgentInfo, Step, Task } from "../../api/types";
import { PATHS } from "../../router";
import { agentKindOf, stepVerb, type AgentKind } from "../../tasks/stage";
import type { TimeAxis } from "./timeAxis";
import {
  conversationBar,
  slowestStep,
  stepBar,
  stepTag,
  taskBar,
  taskGlyph,
  taskMeta,
  taskTag,
  type Bar,
  type GlyphState,
  type Tag,
} from "./traceBars";
import type { ScopedConversation } from "./traceScope";

export type RowKind = "conversation" | "task" | "step" | "note";
export type RowDepth = 0 | 1 | 2;

export interface TraceRow {
  readonly key: string;
  readonly kind: RowKind;
  readonly depth: RowDepth;
  readonly name: string;
  readonly meta: string | null;
  readonly glyph: GlyphState | null;
  readonly bar: Bar | null;
  readonly tag: Tag | null;
  readonly expanded: boolean | null;
  readonly parent: string | null;
  readonly href: string;
}

export interface StepsEntry {
  readonly steps: readonly Step[];
  readonly loading: boolean;
  readonly error: string | null;
}

export type StepsBook = ReadonlyMap<string, StepsEntry>;

export interface RowsInput {
  readonly entries: readonly ScopedConversation[];
  readonly book: StepsBook;
  readonly agents: readonly AgentInfo[];
  readonly types?: readonly AgentKind[];
  readonly toggled: ReadonlySet<string>;
  readonly axis: TimeAxis;
}

const NO_STEPS: readonly Step[] = [];

export const bookKey = (name: string) => name.toLowerCase();
export const conversationKey = (id: string) => `c:${id}`;
export const taskKey = (name: string) => `t:${bookKey(name)}`;

export function isExpanded(key: string, toggled: ReadonlySet<string>): boolean {
  return key.startsWith("c:") !== toggled.has(key);
}

export function toggleKey(toggled: ReadonlySet<string>, key: string): ReadonlySet<string> {
  return toggled.has(key) ? new Set([...toggled].filter((k) => k !== key)) : new Set([...toggled, key]);
}

export function taskHref(task: Task): string {
  return `${PATHS.conversation(task.conversationId)}?t=${encodeURIComponent(task.name)}`;
}

export function stepsOf(book: StepsBook, name: string): readonly Step[] {
  return book.get(bookKey(name))?.steps ?? NO_STEPS;
}

export function latestStep(book: StepsBook, name: string): Step | null {
  return stepsOf(book, name).at(-1) ?? null;
}

function agentOf(agents: readonly AgentInfo[], task: Task): AgentInfo | null {
  const same = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();
  return agents.find((agent) => same(agent.name, task.name) && same(agent.repo, task.repo)) ?? null;
}

function noteText(task: Task, entry: StepsEntry | undefined): string | null {
  if (task.state === "queued") return "no steps yet";
  if (entry === undefined || (entry.loading && entry.steps.length === 0)) return "loading steps…";
  if (entry.steps.length > 0) return null;
  return entry.error ? `error: ${entry.error}` : "no steps yet";
}

function stepRows(task: Task, input: RowsInput): readonly TraceRow[] {
  const entry = input.book.get(bookKey(task.name));
  const base = { depth: 2 as const, glyph: null, expanded: null, parent: taskKey(task.name), href: taskHref(task) };
  const note = noteText(task, entry);
  if (note !== null) {
    return [{ ...base, key: `n:${bookKey(task.name)}`, kind: "note", name: note, meta: null, bar: null, tag: null }];
  }
  const steps = entry?.steps ?? NO_STEPS;
  const slowest = slowestStep(steps);
  return steps.map((step) => ({
    ...base,
    key: `s:${bookKey(task.name)}:${step.id}`,
    kind: "step",
    name: stepVerb(step),
    meta: step.summary,
    bar: stepBar(step, input.axis),
    tag: stepTag(step, step.id === slowest),
  }));
}

function taskRows(task: Task, input: RowsInput): readonly TraceRow[] {
  const key = taskKey(task.name);
  const expanded = isExpanded(key, input.toggled);
  const row: TraceRow = {
    key,
    kind: "task",
    depth: 1,
    name: task.name,
    meta: taskMeta(task, latestStep(input.book, task.name), agentKindOf(input.types ?? [], task)),
    glyph: taskGlyph(task),
    bar: taskBar(task, stepsOf(input.book, task.name), agentOf(input.agents, task), input.axis),
    tag: taskTag(task),
    expanded,
    parent: conversationKey(task.conversationId),
    href: taskHref(task),
  };
  return expanded ? [row, ...stepRows(task, input)] : [row];
}

function conversationRows(entry: ScopedConversation, input: RowsInput): readonly TraceRow[] {
  const key = conversationKey(entry.conversation.id);
  const expanded = isExpanded(key, input.toggled);
  const row: TraceRow = {
    key,
    kind: "conversation",
    depth: 0,
    name: entry.conversation.title,
    meta: entry.repo,
    glyph: null,
    bar: conversationBar(entry, input.axis),
    tag: null,
    expanded,
    parent: null,
    href: PATHS.conversation(entry.conversation.id),
  };
  return expanded ? [row, ...entry.tasks.flatMap((task) => taskRows(task, input))] : [row];
}

export function buildRows(input: RowsInput): readonly TraceRow[] {
  return input.entries.flatMap((entry) => conversationRows(entry, input));
}

export function rowLabel(row: TraceRow): string {
  return [row.name, row.meta, row.tag?.text, row.bar?.label].filter((part) => part).join(" ");
}
