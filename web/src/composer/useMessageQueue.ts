import { useCallback, useEffect, useRef, useState } from "react";
import { toMessage } from "../api/errors";
import { enqueue, removeQueued, useQueue, type QueuedMessage } from "./queue";
import { readReply, writeReply, type ReplyTarget } from "./replies";
import type { SubmitHandler } from "./useSubmit";

export interface MessageQueueOptions {
  readonly conversationId: string | null;
  readonly busy: boolean;
  readonly disabled: boolean;
  readonly onSubmit: SubmitHandler;
}

export interface MessageQueue {
  readonly items: readonly QueuedMessage[];
  readonly editing: QueuedMessage | null;
  readonly error: string | null;
  readonly add: (text: string, attachmentIds: readonly string[], reply: ReplyTarget | null) => boolean;
  readonly remove: (id: number) => void;
  readonly pullLast: () => QueuedMessage | null;
  readonly restore: () => boolean;
  readonly finishEdit: () => void;
  readonly clearError: () => void;
}

async function sendQueued(
  conversationId: string | null,
  message: QueuedMessage,
  onSubmit: SubmitHandler,
): Promise<void> {
  const before = conversationId === null ? null : readReply(conversationId);
  if (conversationId !== null) writeReply(conversationId, message.reply);
  let call: Promise<unknown>;
  try {
    call = onSubmit(message.text, message.attachmentIds, []);
  } finally {
    if (conversationId !== null) writeReply(conversationId, before);
  }
  await call;
}

export function useMessageQueue({ conversationId, busy, disabled, onSubmit }: MessageQueueOptions): MessageQueue {
  const items = useQueue(conversationId);
  const [editing, setEditing] = useState<QueuedMessage | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stalled, setStalled] = useState(false);
  const inFlight = useRef(false);
  const afterAccept = useRef(false);
  const busyNow = useRef(busy);
  const submitNow = useRef(onSubmit);

  useEffect(() => {
    busyNow.current = busy;
    submitNow.current = onSubmit;
  }, [busy, onSubmit]);

  useEffect(() => {
    if (busy) {
      afterAccept.current = false;
      return;
    }
    const head = items[0];
    if (head === undefined || disabled || stalled || inFlight.current || afterAccept.current) return;
    inFlight.current = true;
    setError(null);
    sendQueued(conversationId, head, submitNow.current)
      .then(() => {
        removeQueued(conversationId, head.id);
        afterAccept.current = !busyNow.current;
      })
      .catch((failure: unknown) => {
        setError(toMessage(failure));
        setStalled(true);
      })
      .finally(() => {
        inFlight.current = false;
      });
  }, [busy, items, disabled, stalled, conversationId]);

  const add = useCallback(
    (text: string, attachmentIds: readonly string[], reply: ReplyTarget | null): boolean => {
      const ids = [...(editing?.attachmentIds ?? []), ...attachmentIds];
      if (text === "" && ids.length === 0) return false;
      enqueue(conversationId, { text, attachmentIds: ids, reply: reply ?? editing?.reply ?? null });
      setEditing(null);
      setStalled(false);
      return true;
    },
    [conversationId, editing],
  );

  const remove = useCallback((id: number) => void removeQueued(conversationId, id), [conversationId]);

  const pullLast = useCallback((): QueuedMessage | null => {
    const last = items.at(-1);
    if (last === undefined || editing !== null) return null;
    removeQueued(conversationId, last.id);
    setEditing(last);
    return last;
  }, [conversationId, editing, items]);

  const restore = useCallback((): boolean => {
    if (editing === null) return false;
    enqueue(conversationId, editing);
    setEditing(null);
    return true;
  }, [conversationId, editing]);

  const finishEdit = useCallback(() => setEditing(null), []);
  const clearError = useCallback(() => setError(null), []);
  return { items, editing, error, add, remove, pullLast, restore, finishEdit, clearError };
}
