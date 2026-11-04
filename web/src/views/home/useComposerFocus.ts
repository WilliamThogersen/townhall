import { useCallback, useEffect, useState, type RefObject } from "react";
import { NEW_CONVERSATION_HINT, isTextField } from "../../shell/globalKeys";

function focusAtEnd(input: HTMLTextAreaElement | null): void {
  if (!input) return;
  input.focus();
  input.setSelectionRange(input.value.length, input.value.length);
}

export function useComposerFocus(inputRef: RefObject<HTMLTextAreaElement | null>): () => void {
  const [request, setRequest] = useState(0);

  useEffect(() => {
    focusAtEnd(inputRef.current);
  }, [inputRef, request]);

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== NEW_CONVERSATION_HINT.keys || e.isComposing) return;
      if (e.metaKey || e.ctrlKey || e.altKey || isTextField(e.target)) return;
      e.preventDefault();
      focusAtEnd(inputRef.current);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [inputRef]);

  return useCallback(() => setRequest((n) => n + 1), []);
}
