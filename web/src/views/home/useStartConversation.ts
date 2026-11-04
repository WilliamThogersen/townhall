import { useCallback, useState } from "react";
import { toMessage } from "../../api/errors";
import { useConversationActions } from "../../data";
import { navigate, PATHS } from "../../router";

export interface StartConversation {
  readonly start: (text: string) => Promise<void>;
  readonly busy: boolean;
  readonly error: string | null;
}

export function useStartConversation(): StartConversation {
  const actions = useConversationActions();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const start = useCallback(
    async (text: string) => {
      const trimmed = text.trim();
      if (trimmed === "") return;
      setBusy(true);
      setError(null);
      try {
        const detail = await actions.start({ text: trimmed });
        navigate(PATHS.conversation(detail.conversation.id));
      } catch (e) {
        setError(toMessage(e));
      } finally {
        setBusy(false);
      }
    },
    [actions],
  );

  return { start, busy, error };
}
