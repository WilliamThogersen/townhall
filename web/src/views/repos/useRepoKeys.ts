import { useEffect, useRef } from "react";
import { isTextField } from "../../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";

export const ADD_REPO_HINT: KeyHint = { keys: "a", label: "add repository" };
export const FORM_HINTS: readonly KeyHint[] = [
  { keys: "⏎", label: "add" },
  { keys: "esc", label: "cancel" },
];

export function useRepoKeys(formOpen: boolean, onAdd: () => void): void {
  const latest = useRef(onAdd);
  useEffect(() => {
    latest.current = onAdd;
  });
  useKeyHints(formOpen ? FORM_HINTS : [ADD_REPO_HINT]);
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== ADD_REPO_HINT.keys || e.defaultPrevented || e.isComposing) return;
      if (e.metaKey || e.ctrlKey || e.altKey || isTextField(e.target)) return;
      e.preventDefault();
      latest.current();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
