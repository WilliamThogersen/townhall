import { useDataStore } from "./dataContext";
import type { ConversationActions } from "./types";

export function useConversationActions(): ConversationActions {
  return useDataStore().actions;
}
