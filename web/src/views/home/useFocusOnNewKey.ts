import { useEffect, type RefObject } from "react";
import type { ComposerHandle } from "../../composer";
import { NEW_CONVERSATION_HINT, isTextField } from "../../shell/globalKeys";

export function useFocusOnNewKey(composer: RefObject<ComposerHandle | null>): void {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== NEW_CONVERSATION_HINT.keys || e.isComposing) return;
      if (e.metaKey || e.ctrlKey || e.altKey || isTextField(e.target)) return;
      e.preventDefault();
      composer.current?.focus();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [composer]);
}
