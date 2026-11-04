import { useCallback, useMemo } from "react";
import type { Conversation } from "../api/types";
import { useDataStore, useResource } from "./dataContext";
import { CONVERSATIONS_KEY, detailKey } from "./resources";
import type { ConversationsState, ConversationState } from "./types";

const NO_CONVERSATIONS: readonly Conversation[] = [];

export function useConversations(): ConversationsState {
  const { data, loading, error } = useResource(CONVERSATIONS_KEY, (state) => state.conversations);
  return useMemo(
    () => ({ conversations: data ?? NO_CONVERSATIONS, loading, error }),
    [data, loading, error],
  );
}

export function useConversation(id: string | null): ConversationState {
  const store = useDataStore();
  const { data, loading, error } = useResource(id === null ? null : detailKey(id), (state) =>
    id === null ? null : state.details.get(id),
  );
  const reload = useCallback(() => {
    if (id !== null) store.refetch(detailKey(id));
  }, [store, id]);
  return useMemo(() => ({ detail: data, loading, error, reload }), [data, loading, error, reload]);
}
