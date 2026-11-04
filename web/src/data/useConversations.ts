import { useMemo } from "react";
import { getConversation, listConversations } from "../api/conversations";
import type { Conversation } from "../api/types";
import { useDataContext } from "./dataContext";
import type { ConversationsState, ConversationState } from "./types";
import { useLoad } from "./useLoad";

const NO_CONVERSATIONS: readonly Conversation[] = [];

export function newestFirst(list: readonly Conversation[]): readonly Conversation[] {
  return [...list].sort((a, b) => b.updatedAt.localeCompare(a.updatedAt));
}

export function useConversations(): ConversationsState {
  const { refresh } = useDataContext();
  const { data, loading, error } = useLoad(
    "conversations",
    (signal) => listConversations(signal),
    refresh,
  );
  const conversations = useMemo(() => (data ? newestFirst(data) : NO_CONVERSATIONS), [data]);
  return { conversations, loading, error };
}

export function useConversation(id: string | null): ConversationState {
  const { refresh } = useDataContext();
  const { data, loading, error, reload } = useLoad(
    id === null ? null : `conversation:${id}`,
    (signal) => getConversation(id ?? "", signal),
    refresh,
  );
  return { detail: data, loading, error, reload };
}
