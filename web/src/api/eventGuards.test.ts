import { describe, expect, it } from "vitest";
import { parseServerEvent } from "./eventGuards";
import { aConversation, anAgentInfo, aTask, aYouEntry } from "./fixtures";
import type { ServerEvent } from "./types";

const events: readonly ServerEvent[] = [
  { type: "conversationUpdated", conversation: aConversation() },
  { type: "entryAppended", conversationId: "c1", entry: aYouEntry(2, "hi") },
  { type: "taskUpdated", task: aTask({ state: "working" }) },
  { type: "agentUpdated", agent: anAgentInfo({ state: { kind: "waiting", question: "go?" } }) },
  { type: "resync" },
];

describe("server event guard", () => {
  it.each(events.map((event) => [event.type, event] as const))("reads %s", (_, event) => {
    expect(parseServerEvent(JSON.parse(JSON.stringify(event)))).toEqual(event);
  });

  it("refuses unknown types and broken payloads", () => {
    expect(() => parseServerEvent({ type: "hello" })).toThrow("event.type");
    expect(() => parseServerEvent("resync")).toThrow("event is not an object");
    expect(() => parseServerEvent({ type: "entryAppended", entry: aYouEntry(1, "x") })).toThrow(
      "event.conversationId",
    );
    expect(() => parseServerEvent({ type: "taskUpdated", task: {} })).toThrow("task.name");
  });
});
