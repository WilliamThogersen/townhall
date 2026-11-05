import {
  parseConversation,
  parseConversationDetail,
  parseConversations,
  parseThreadEntry,
} from "./conversationGuards";
import { request } from "./request";
import type {
  Conversation,
  ConversationDetail,
  ConversationPatch,
  NewMessageInput,
  ThreadEntry,
} from "./types";

const path = (id: string, rest = ""): string => `/conversations/${encodeURIComponent(id)}${rest}`;

export const listConversations = (signal?: AbortSignal): Promise<readonly Conversation[]> =>
  request("GET", "/conversations", parseConversations, undefined, signal);

export const getConversation = (id: string, signal?: AbortSignal): Promise<ConversationDetail> =>
  request("GET", path(id), parseConversationDetail, undefined, signal);

function withFiles(input: NewMessageInput, files: readonly File[]): NewMessageInput | FormData {
  if (files.length === 0) return input;
  const form = new FormData();
  form.append("text", input.text);
  files.forEach((file) => form.append("file", file, file.name));
  return form;
}

export const startConversation = (
  input: NewMessageInput,
  files: readonly File[] = [],
): Promise<ConversationDetail> =>
  request("POST", "/conversations", parseConversationDetail, withFiles(input, files));

export const sendMessage = (id: string, input: NewMessageInput): Promise<ThreadEntry> =>
  request("POST", path(id, "/messages"), parseThreadEntry, input);

export const patchConversation = (id: string, patch: ConversationPatch): Promise<Conversation> =>
  request("PATCH", path(id), parseConversation, patch);

export const stopTurn = (id: string): Promise<Conversation> =>
  request("POST", path(id, "/stop"), parseConversation);

export const endConversation = (id: string): Promise<Conversation> =>
  request("POST", path(id, "/end"), parseConversation);
