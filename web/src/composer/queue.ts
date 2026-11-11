import { useSyncExternalStore } from "react";
import type { ReplyTarget } from "./replies";

export interface QueuedMessage {
  readonly id: number;
  readonly text: string;
  readonly attachmentIds: readonly string[];
  readonly reply: ReplyTarget | null;
}

export type NewQueuedMessage = Omit<QueuedMessage, "id"> & { readonly id?: number };

const HOME = "\0home";
const NONE: readonly QueuedMessage[] = [];
const queues = new Map<string, readonly QueuedMessage[]>();
const listeners = new Set<() => void>();
let counter = 0;

const keyOf = (conversationId: string | null): string => conversationId ?? HOME;

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => void listeners.delete(listener);
}

function changed(): void {
  listeners.forEach((listener) => listener());
}

function store(conversationId: string | null, next: readonly QueuedMessage[]): void {
  if (next.length === 0) queues.delete(keyOf(conversationId));
  else queues.set(keyOf(conversationId), next);
  changed();
}

export function readQueue(conversationId: string | null): readonly QueuedMessage[] {
  return queues.get(keyOf(conversationId)) ?? NONE;
}

export function enqueue(conversationId: string | null, message: NewQueuedMessage): QueuedMessage {
  counter += 1;
  const queued: QueuedMessage = { ...message, id: message.id ?? counter };
  store(conversationId, [...readQueue(conversationId), queued]);
  return queued;
}

export function removeQueued(conversationId: string | null, id: number): QueuedMessage | null {
  const current = readQueue(conversationId);
  const found = current.find((item) => item.id === id) ?? null;
  if (found !== null) {
    store(
      conversationId,
      current.filter((item) => item.id !== id),
    );
  }
  return found;
}

export function clearAllQueues(): void {
  queues.clear();
  changed();
}

export function useQueue(conversationId: string | null): readonly QueuedMessage[] {
  return useSyncExternalStore(subscribe, () => readQueue(conversationId));
}
