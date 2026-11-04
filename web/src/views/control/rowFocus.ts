import type { KeyboardEvent } from "react";

const ROW_SELECTOR = "button[data-agent]";

const ROW_STEP: Readonly<Record<string, number>> = { ArrowDown: 1, ArrowUp: -1 };

export function rowButtons(list: HTMLElement | null): HTMLButtonElement[] {
  return list ? [...list.querySelectorAll<HTMLButtonElement>(ROW_SELECTOR)] : [];
}

export function focusAgentRow(list: HTMLElement | null, name: string): boolean {
  const target = rowButtons(list).find((button) => button.dataset.agent === name);
  target?.focus();
  return target !== undefined;
}

export function focusCurrentRow(list: HTMLElement | null): boolean {
  const buttons = rowButtons(list);
  const target = buttons.find((button) => button.tabIndex === 0) ?? buttons[0];
  target?.focus();
  return target !== undefined;
}

export function moveRowFocus(e: KeyboardEvent<HTMLElement>): void {
  const step = ROW_STEP[e.key];
  if (step === undefined) return;
  const buttons = rowButtons(e.currentTarget);
  const index = buttons.indexOf(e.target as HTMLButtonElement);
  if (index < 0) return;
  e.preventDefault();
  const next = Math.min(buttons.length - 1, Math.max(0, index + step));
  buttons[next]?.focus();
}
