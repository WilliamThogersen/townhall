import { useEffect, type KeyboardEvent as ReactKeyboardEvent, type RefObject } from "react";
import { useKeyHints, type KeyHint } from "../../../shell/keyHints";
import { isPlainKey } from "./keys";

export const TASK_ROWS_HINT: KeyHint = { keys: "j k", label: "tasks" };
export const OPEN_TASK_HINT: KeyHint = { keys: "⏎", label: "open terminal" };
export const SESSION_KEY_HINTS: readonly KeyHint[] = [TASK_ROWS_HINT, OPEN_TASK_HINT];

const ROW_SELECTOR = "button[data-task]";
const LETTER_STEPS: Readonly<Record<string, number>> = { j: 1, k: -1 };
const ARROW_STEPS: Readonly<Record<string, number>> = { ArrowDown: 1, ArrowUp: -1 };

function taskButtons(list: HTMLElement | null): HTMLButtonElement[] {
  return list ? [...list.querySelectorAll<HTMLButtonElement>(ROW_SELECTOR)] : [];
}

function startIndex(buttons: readonly HTMLButtonElement[]): number {
  return Math.max(0, buttons.findIndex((button) => button.getAttribute("aria-pressed") === "true"));
}

export function stepTaskFocus(list: HTMLElement | null, step: number): boolean {
  const buttons = taskButtons(list);
  if (buttons.length === 0) return false;
  const index = buttons.indexOf(document.activeElement as HTMLButtonElement);
  const next = index < 0 ? startIndex(buttons) : Math.min(buttons.length - 1, Math.max(0, index + step));
  buttons[next]?.focus();
  return true;
}

export function onTaskListKeyDown(e: ReactKeyboardEvent<HTMLElement>): void {
  const step = ARROW_STEPS[e.key];
  if (step === undefined || !(e.target instanceof HTMLButtonElement) || !e.target.matches(ROW_SELECTOR)) return;
  e.preventDefault();
  stepTaskFocus(e.currentTarget, step);
}

export function TaskKeyHints() {
  useKeyHints(SESSION_KEY_HINTS);
  return null;
}

export function useTaskKeys(listRef: RefObject<HTMLElement | null>): void {
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const step = LETTER_STEPS[e.key];
      if (step === undefined || !isPlainKey(e)) return;
      if (stepTaskFocus(listRef.current, step)) e.preventDefault();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [listRef]);
}
