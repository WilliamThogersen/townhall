import type { KeyboardEvent } from "react";

const FOCUSABLE = [
  "a[href]",
  "button:not([disabled])",
  "input:not([disabled])",
  "select:not([disabled])",
  "textarea:not([disabled])",
  '[tabindex]:not([tabindex="-1"])',
].join(",");

function focusables(root: HTMLElement): readonly HTMLElement[] {
  return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE));
}

export function trapTab(event: KeyboardEvent, root: HTMLElement): void {
  if (event.key !== "Tab") return;
  const items = focusables(root);
  const first = items[0];
  const last = items[items.length - 1];
  if (first === undefined || last === undefined) return;
  const active = document.activeElement;
  const wrapTo = event.shiftKey ? (active === first ? last : null) : active === last ? first : null;
  if (wrapTo === null) return;
  event.preventDefault();
  wrapTo.focus();
}
