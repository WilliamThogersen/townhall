import { useSyncExternalStore } from "react";

export interface ReplyTarget {
  readonly id: number;
  readonly author: string;
  readonly excerpt: string;
}

const replies = new Map<string, ReplyTarget>();
const listeners = new Set<() => void>();

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

function changed(): void {
  listeners.forEach((listener) => listener());
}

export function readReply(conversationId: string | null): ReplyTarget | null {
  return conversationId === null ? null : (replies.get(conversationId) ?? null);
}

export function writeReply(conversationId: string, target: ReplyTarget | null): void {
  if (target === null) replies.delete(conversationId);
  else replies.set(conversationId, target);
  changed();
}

export function clearAllReplies(): void {
  replies.clear();
  changed();
}

export function useReply(conversationId: string | null): ReplyTarget | null {
  return useSyncExternalStore(subscribe, () => readReply(conversationId));
}
