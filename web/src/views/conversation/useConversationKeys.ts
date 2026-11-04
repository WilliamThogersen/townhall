import { useEffect, useRef } from "react";
import { isTextField } from "../../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";

export const REPLY_HINT: KeyHint = { keys: "r", label: "reply" };
export const SESSION_HINT: KeyHint = { keys: "s", label: "session panel" };

export const CONVERSATION_HINTS: readonly KeyHint[] = [
  REPLY_HINT,
  SESSION_HINT,
  { keys: "⏎", label: "send" },
  { keys: "⇧⏎", label: "new line" },
  { keys: "esc", label: "leave input" },
];

export interface ConversationKeys {
  readonly onToggleSession: () => void;
  readonly onFocusComposer: () => void;
}

function inDialog(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest("dialog") !== null;
}

function ignored(e: KeyboardEvent): boolean {
  if (e.defaultPrevented || e.isComposing) return true;
  if (e.metaKey || e.ctrlKey || e.altKey) return true;
  return isTextField(e.target) || inDialog(e.target);
}

export function useConversationKeys(keys: ConversationKeys): void {
  useKeyHints(CONVERSATION_HINTS);
  const latest = useRef(keys);
  useEffect(() => {
    latest.current = keys;
  });
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (ignored(e)) return;
      const action =
        e.key === SESSION_HINT.keys
          ? latest.current.onToggleSession
          : e.key === REPLY_HINT.keys
            ? latest.current.onFocusComposer
            : null;
      if (action === null) return;
      e.preventDefault();
      action();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
