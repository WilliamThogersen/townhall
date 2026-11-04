import { useEffect, useRef } from "react";
import { isTextField } from "../../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";

export const ROWS_HINT: KeyHint = { keys: "↑↓", label: "rows" };
export const OPEN_HINT: KeyHint = { keys: "⏎", label: "open agent" };
export const CLOSE_HINT: KeyHint = { keys: "esc", label: "close pane" };

const ROW_KEYS: ReadonlySet<string> = new Set(["ArrowDown", "ArrowUp"]);

export function controlHints(hasRows: boolean, paneOpen: boolean): readonly KeyHint[] {
  return [...(hasRows ? [ROWS_HINT, OPEN_HINT] : []), ...(paneOpen ? [CLOSE_HINT] : [])];
}

export interface ControlKeys {
  readonly hasRows: boolean;
  readonly paneOpen: boolean;
  readonly onClosePane: () => void;
  readonly onFocusRows: () => void;
}

function isPlainKey(e: KeyboardEvent): boolean {
  if (e.defaultPrevented || e.isComposing) return false;
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  return !isTextField(e.target);
}

export function useControlKeys(keys: ControlKeys): void {
  const latest = useRef(keys);
  useEffect(() => {
    latest.current = keys;
  });
  useKeyHints(controlHints(keys.hasRows, keys.paneOpen));

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!isPlainKey(e)) return;
      const { paneOpen, hasRows, onClosePane, onFocusRows } = latest.current;
      if (e.key === "Escape" && paneOpen) {
        e.preventDefault();
        onClosePane();
      } else if (ROW_KEYS.has(e.key) && hasRows && e.target === document.body) {
        e.preventDefault();
        onFocusRows();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
