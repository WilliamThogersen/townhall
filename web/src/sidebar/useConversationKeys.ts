import { useEffect, useRef } from "react";
import { navigate, PATHS } from "../router";

export const OVERLAY_SELECTOR = '[role="menu"], [role="dialog"]';

const STEPS: ReadonlyMap<string, number> = new Map([
  ["ArrowUp", -1],
  ["ArrowDown", 1],
]);

export function insideOverlay(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(OVERLAY_SELECTOR) !== null;
}

export function nextConversation(ids: readonly string[], currentId: string | null, step: number): string | null {
  if (ids.length === 0) return null;
  const at = currentId === null ? -1 : ids.indexOf(currentId);
  if (at < 0) return (step > 0 ? ids[0] : ids.at(-1)) ?? null;
  const next = ids[at + step];
  return next ?? null;
}

export function useConversationKeys(ids: readonly string[], currentId: string | null): void {
  const latest = useRef({ ids, currentId });
  useEffect(() => {
    latest.current = { ids, currentId };
  });
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const step = STEPS.get(e.key);
      if (step === undefined || !e.altKey || e.metaKey || e.ctrlKey || e.shiftKey) return;
      if (e.defaultPrevented || e.isComposing || insideOverlay(e.target)) return;
      const next = nextConversation(latest.current.ids, latest.current.currentId, step);
      e.preventDefault();
      if (next !== null) navigate(PATHS.conversation(next));
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
