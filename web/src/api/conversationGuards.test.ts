import { describe, expect, it } from "vitest";
import {
  parseAttachment,
  parseConversation,
  parseConversationDetail,
  parseThreadEntry,
} from "./conversationGuards";
import { aConversation, aDetail, anAttachment, anEntry, aTask } from "./fixtures";
import type { EntryBody } from "./types";

const bodies: readonly EntryBody[] = [
  { kind: "you", text: "hi", attachments: ["a1"] },
  { kind: "townhall", text: "**ok**" },
  { kind: "repo", repo: "shop", baseBranch: "develop", rule: "code", reason: "it has src/a.ts" },
  { kind: "repoAsk", question: "which one?", candidates: ["shop", "admin"], offerAll: false },
  { kind: "tasksCreated", tasks: ["fix-login"] },
  { kind: "needsYou", agent: "fix-login", question: "go?", choices: ["y", "n"] },
  { kind: "agent", agent: "fix-login", event: "passed", text: "merged into main" },
  { kind: "handoff", task: "fix-login", from: "shipper", to: "coder", reason: "lint fails" },
  { kind: "error", message: "claude exited with 1" },
];

describe("conversation guards", () => {
  it("reads a conversation with and without context", () => {
    expect(parseConversation(aConversation())).toEqual(aConversation());
    const busy = aConversation({ status: "thinking", context: { used: 10, limit: 200 }, linkedRepo: "shop" });
    expect(parseConversation(busy)).toEqual(busy);
  });

  it("refuses a conversation with a wrong field", () => {
    expect(() => parseConversation({ ...aConversation(), status: "sleeping" })).toThrow("conversation.status");
    expect(() => parseConversation({ ...aConversation(), tasks: "a" })).toThrow("conversation.tasks");
    expect(() => parseConversation({ ...aConversation(), context: { used: 1 } })).toThrow("context.limit");
  });

  it.each(bodies.map((body) => [body.kind, body] as const))("reads a %s entry", (_, body) => {
    const entry = anEntry(4, body);
    expect(parseThreadEntry(entry)).toEqual(entry);
  });

  it("refuses unknown kinds and missing fields", () => {
    expect(() => parseThreadEntry({ id: 1, at: "t", kind: "toString" })).toThrow("entry.kind");
    expect(() => parseThreadEntry({ id: 1, at: "t" })).toThrow("entry.kind");
    expect(() => parseThreadEntry({ id: "1", at: "t", kind: "townhall", text: "x" })).toThrow("entry.id");
    expect(() => parseThreadEntry({ id: 1, at: "t", kind: "agent", agent: "a", event: "x", text: "" }))
      .toThrow("entry.event");
    expect(() => parseThreadEntry({ id: 1, at: "t", kind: "you", text: "x", attachments: [1] }))
      .toThrow("entry.attachments[0]");
  });

  it("reads attachments and refuses an unknown kind", () => {
    expect(parseAttachment(anAttachment())).toEqual(anAttachment());
    expect(() => parseAttachment({ ...anAttachment(), kind: "video" })).toThrow("attachment.kind");
  });

  it("reads a whole detail", () => {
    const detail = aDetail({ entries: [anEntry(1, bodies[0]!)], tasks: [aTask()], attachments: [anAttachment()] });
    expect(parseConversationDetail(detail)).toEqual(detail);
    expect(() => parseConversationDetail({ ...detail, tasks: null })).toThrow("detail.tasks is not an array");
  });
});
