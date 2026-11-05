import { useEffect, useRef } from "react";
import type { KeyHint } from "../../../shell/keyHints";
import { isInTerminal, isPlainKey } from "./keys";

export const CLOSE_TERMINAL_HINT: KeyHint = { keys: "esc", label: "close terminal" };

export function useCloseKey(onClose: () => void): void {
  const latest = useRef(onClose);
  useEffect(() => {
    latest.current = onClose;
  });

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || !isPlainKey(e) || isInTerminal(e.target)) return;
      e.preventDefault();
      latest.current();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
