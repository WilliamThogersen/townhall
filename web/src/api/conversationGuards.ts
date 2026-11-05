import {
  bool,
  fail,
  list,
  listOf,
  nullable,
  num,
  oneOf,
  record,
  str,
  strings,
  type Fields,
  type Guard,
} from "./guardKit";
import { parseTask } from "./taskGuards";
import type {
  AgentEvent,
  Attachment,
  AttachmentKind,
  ContextUsage,
  Conversation,
  ConversationDetail,
  ConversationStatus,
  EntryBody,
  RepoRule,
  ThreadEntry,
} from "./types";

const STATUSES = ["thinking", "yourTurn"] as const satisfies readonly ConversationStatus[];
const REPO_RULES = ["named", "code", "only", "picked", "linked"] as const satisfies readonly RepoRule[];
const AGENT_EVENTS = ["sent", "passed", "failed", "ended", "allowed"] as const satisfies readonly AgentEvent[];
const ATTACHMENT_KINDS = ["image", "file"] as const satisfies readonly AttachmentKind[];

export function parseContextUsage(value: unknown): ContextUsage {
  const r = record(value, "conversation.context");
  return { used: num(r.used, "context.used"), limit: num(r.limit, "context.limit") };
}

export function parseConversation(value: unknown): Conversation {
  const r = record(value, "conversation");
  return {
    id: str(r.id, "conversation.id"),
    sessionId: str(r.sessionId, "conversation.sessionId"),
    sessionStarted: bool(r.sessionStarted, "conversation.sessionStarted"),
    title: str(r.title, "conversation.title"),
    status: oneOf(STATUSES)(r.status, "conversation.status"),
    tasks: strings(r.tasks, "conversation.tasks"),
    linkedRepo: nullable(r.linkedRepo, "conversation.linkedRepo", str),
    ended: bool(r.ended, "conversation.ended"),
    context: nullable(r.context, "conversation.context", parseContextUsage),
    createdAt: str(r.createdAt, "conversation.createdAt"),
    updatedAt: str(r.updatedAt, "conversation.updatedAt"),
  };
}

export const parseConversations: Guard<readonly Conversation[]> = listOf(
  parseConversation,
  "conversations",
);

type BodyParser = (r: Fields) => EntryBody;

const BODY_PARSERS: Readonly<Record<EntryBody["kind"], BodyParser>> = {
  you: (r) => ({
    kind: "you",
    text: str(r.text, "entry.text"),
    attachments: strings(r.attachments, "entry.attachments"),
  }),
  townhall: (r) => ({ kind: "townhall", text: str(r.text, "entry.text") }),
  repo: (r) => ({
    kind: "repo",
    repo: str(r.repo, "entry.repo"),
    baseBranch: str(r.baseBranch, "entry.baseBranch"),
    rule: oneOf(REPO_RULES)(r.rule, "entry.rule"),
    reason: str(r.reason, "entry.reason"),
  }),
  repoAsk: (r) => ({
    kind: "repoAsk",
    question: str(r.question, "entry.question"),
    candidates: strings(r.candidates, "entry.candidates"),
    offerAll: bool(r.offerAll, "entry.offerAll"),
  }),
  tasksCreated: (r) => ({ kind: "tasksCreated", tasks: strings(r.tasks, "entry.tasks") }),
  needsYou: (r) => ({
    kind: "needsYou",
    agent: str(r.agent, "entry.agent"),
    question: str(r.question, "entry.question"),
    choices: strings(r.choices, "entry.choices"),
  }),
  agent: (r) => ({
    kind: "agent",
    agent: str(r.agent, "entry.agent"),
    event: oneOf(AGENT_EVENTS)(r.event, "entry.event"),
    text: str(r.text, "entry.text"),
  }),
  error: (r) => ({ kind: "error", message: str(r.message, "entry.message") }),
};

function isEntryKind(kind: unknown): kind is EntryBody["kind"] {
  return typeof kind === "string" && Object.hasOwn(BODY_PARSERS, kind);
}

export function parseEntryBody(value: unknown): EntryBody {
  const r = record(value, "entry");
  return isEntryKind(r.kind) ? BODY_PARSERS[r.kind](r) : fail("entry.kind");
}

export function parseThreadEntry(value: unknown): ThreadEntry {
  const r = record(value, "entry");
  const body = parseEntryBody(r);
  return { id: num(r.id, "entry.id"), at: str(r.at, "entry.at"), ...body };
}

export function parseAttachment(value: unknown): Attachment {
  const r = record(value, "attachment");
  return {
    id: str(r.id, "attachment.id"),
    conversationId: str(r.conversationId, "attachment.conversationId"),
    name: str(r.name, "attachment.name"),
    mime: str(r.mime, "attachment.mime"),
    size: num(r.size, "attachment.size"),
    kind: oneOf(ATTACHMENT_KINDS)(r.kind, "attachment.kind"),
    createdAt: str(r.createdAt, "attachment.createdAt"),
  };
}

export const parseAttachments: Guard<readonly Attachment[]> = listOf(parseAttachment, "attachments");

export function parseConversationDetail(value: unknown): ConversationDetail {
  const r = record(value, "detail");
  return {
    conversation: parseConversation(r.conversation),
    entries: list(r.entries, "detail.entries", parseThreadEntry),
    tasks: list(r.tasks, "detail.tasks", parseTask),
    attachments: list(r.attachments, "detail.attachments", parseAttachment),
  };
}
