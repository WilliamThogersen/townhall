import { useCallback } from "react";
import { useConversationActions } from "../../data";
import { navigate, PATHS } from "../../router";

export function useStartConversation(): (text: string) => Promise<void> {
  const actions = useConversationActions();
  return useCallback(
    async (text: string) => {
      const detail = await actions.start({ text });
      navigate(PATHS.conversation(detail.conversation.id));
    },
    [actions],
  );
}
