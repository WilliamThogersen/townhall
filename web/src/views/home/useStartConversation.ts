import { useCallback } from "react";
import { useConversationActions } from "../../data";
import { navigate, PATHS } from "../../router";

export function useStartConversation(): (
  text: string,
  attachmentIds: readonly string[],
  files: readonly File[],
) => Promise<void> {
  const actions = useConversationActions();
  return useCallback(
    async (text, _attachmentIds, files) => {
      const detail = await actions.start({ text }, files);
      navigate(PATHS.conversation(detail.conversation.id));
    },
    [actions],
  );
}
