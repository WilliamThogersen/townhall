import { useMemo } from "react";
import {
  endConversation,
  patchConversation,
  sendMessage,
  startConversation,
  stopTurn,
} from "../api/conversations";
import { useDataContext } from "./dataContext";
import type { ConversationActions } from "./types";

export function useConversationActions(): ConversationActions {
  useDataContext();
  return useMemo(
    () => ({
      start: (input) => startConversation(input),
      send: (id, input) => sendMessage(id, input),
      stop: (id) => stopTurn(id),
      end: (id) => endConversation(id),
      patch: (id, patch) => patchConversation(id, patch),
    }),
    [],
  );
}
