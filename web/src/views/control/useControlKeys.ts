import { useEffect, useRef } from "react";
import { isTextField } from "../../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../../shell/keyHints";

export const ROWS_HINT: KeyHint = { keys: "↑↓", label: "rows" };
export const FOLD_HINT: KeyHint = { keys: "←→", label: "fold" };
export const OPEN_HINT: KeyHint = { keys: "⏎", label: "open conversation" };
export const FILTER_HINT: KeyHint = { keys: "f", label: "filter repo" };

const ROW_KEYS: ReadonlySet<string> = new Set(["ArrowDown", "ArrowUp"]);
export const FILTER_KEY = "f";

export function controlHints(hasRows: boolean): readonly KeyHint[] {
  return hasRows ? [ROWS_HINT, FOLD_HINT, OPEN_HINT, FILTER_HINT] : [FILTER_HINT];
}

export interface ControlKeys {
  readonly hasRows: boolean;
  readonly onFocusRows: () => void;
  readonly onFilter: () => void;
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
  useKeyHints(controlHints(keys.hasRows));

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!isPlainKey(e)) return;
      const { hasRows, onFocusRows, onFilter } = latest.current;
      if (e.key === FILTER_KEY) {
        e.preventDefault();
        onFilter();
      } else if (ROW_KEYS.has(e.key) && hasRows && e.target === document.body) {
        e.preventDefault();
        onFocusRows();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
