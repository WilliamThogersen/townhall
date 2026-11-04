import { parseConversation, parseThreadEntry } from "./conversationGuards";
import { fail, record, str } from "./guardKit";
import { parseAgentInfo } from "./guards";
import { parseTask } from "./taskGuards";
import type { ServerEvent } from "./types";

export function parseServerEvent(value: unknown): ServerEvent {
  const r = record(value, "event");
  switch (r.type) {
    case "conversationUpdated":
      return { type: "conversationUpdated", conversation: parseConversation(r.conversation) };
    case "entryAppended":
      return {
        type: "entryAppended",
        conversationId: str(r.conversationId, "event.conversationId"),
        entry: parseThreadEntry(r.entry),
      };
    case "taskUpdated":
      return { type: "taskUpdated", task: parseTask(r.task) };
    case "agentUpdated":
      return { type: "agentUpdated", agent: parseAgentInfo(r.agent) };
    case "resync":
      return { type: "resync" };
    default:
      return fail("event.type");
  }
}
