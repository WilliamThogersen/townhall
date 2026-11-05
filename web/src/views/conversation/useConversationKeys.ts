import { useEffect, useRef, useState } from "react";
import { isInTerminal, isTextField } from "../../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";
import { CLOSE_TERMINAL_HINT, SESSION_KEY_HINTS } from "./session";

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
  readonly panelKeys: boolean;
  readonly terminalOpen: boolean;
}

/** While the terminal has focus esc goes to the program in it (it interrupts claude). */
export const TERMINAL_FOCUSED_HINT: KeyHint = { keys: "esc", label: "interrupt claude" };

export function conversationHints(
  panelKeys: boolean,
  terminalOpen: boolean,
  terminalFocused = false,
): readonly KeyHint[] {
  return [
    ...CONVERSATION_HINTS,
    ...(panelKeys ? SESSION_KEY_HINTS : []),
    ...(terminalOpen ? [terminalFocused ? TERMINAL_FOCUSED_HINT : CLOSE_TERMINAL_HINT] : []),
  ];
}

/** Whether keyboard focus is inside the live terminal. */
function useTerminalFocused(): boolean {
  const [focused, setFocused] = useState(false);
  useEffect(() => {
    const update = () => setFocused(isInTerminal(document.activeElement));
    document.addEventListener("focusin", update);
    document.addEventListener("focusout", update);
    return () => {
      document.removeEventListener("focusin", update);
      document.removeEventListener("focusout", update);
    };
  }, []);
  return focused;
}

function inDialog(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest("dialog") !== null;
}

function ignored(e: KeyboardEvent): boolean {
  if (e.defaultPrevented || e.isComposing || isInTerminal(e.target)) return true;
  if (e.metaKey || e.ctrlKey || e.altKey) return true;
  return isTextField(e.target) || inDialog(e.target);
}

export function useConversationKeys(keys: ConversationKeys): void {
  const terminalFocused = useTerminalFocused();
  useKeyHints(conversationHints(keys.panelKeys, keys.terminalOpen, terminalFocused));
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
