import { useEffect, useRef } from "react";
import { navigate, PATHS } from "../router";
import type { KeyHint } from "./keyHints";

export const NEW_CONVERSATION_HINT: KeyHint = { keys: "n", label: "new conversation" };
export const SEARCH_HINT: KeyHint = { keys: "/", label: "search" };
export const HELP_HINT: KeyHint = { keys: "?", label: "help" };
export const MENU_HINT: KeyHint = { keys: "m", label: "menu" };

const GLOBAL_KEYS: ReadonlyMap<string, string> = new Map([
  [NEW_CONVERSATION_HINT.keys, PATHS.home],
  [SEARCH_HINT.keys, PATHS.search],
]);

const NON_TEXT_INPUTS: ReadonlySet<string> = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "hidden",
  "image",
  "radio",
  "range",
  "reset",
  "submit",
]);

const EDITABLE = '[contenteditable]:not([contenteditable="false"])';

export function isTextField(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target.isContentEditable || target.closest(EDITABLE)) return true;
  if (target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement) return true;
  return target instanceof HTMLInputElement && !NON_TEXT_INPUTS.has(target.type);
}

/** Keys typed into the live agent terminal belong to the program in it, never to the app. */
export function isInTerminal(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest(".th-term") !== null;
}

function inModal(target: EventTarget | null): boolean {
  return target instanceof Element && target.closest('[aria-modal="true"]') !== null;
}

function plainKey(e: KeyboardEvent): boolean {
  if (e.defaultPrevented || e.isComposing || isInTerminal(e.target)) return false;
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  return !isTextField(e.target) && !inModal(e.target);
}

export function useGlobalKeys(onHelp?: () => void, onMenu?: () => void): void {
  const latest = useRef(onHelp);
  const latestMenu = useRef(onMenu);
  useEffect(() => {
    latest.current = onHelp;
    latestMenu.current = onMenu;
  });
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!plainKey(e)) return;
      if (e.key === HELP_HINT.keys && latest.current !== undefined) {
        e.preventDefault();
        latest.current();
        return;
      }
      if (e.key === MENU_HINT.keys && latestMenu.current !== undefined) {
        e.preventDefault();
        latestMenu.current();
        return;
      }
      const to = GLOBAL_KEYS.get(e.key);
      if (to === undefined) return;
      e.preventDefault();
      navigate(to);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);
}
