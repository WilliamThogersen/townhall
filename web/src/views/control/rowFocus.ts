const ROW_SELECTOR = "[data-row]";

export function rowElements(tree: HTMLElement | null): HTMLElement[] {
  return tree ? [...tree.querySelectorAll<HTMLElement>(ROW_SELECTOR)] : [];
}

export function focusRow(tree: HTMLElement | null, key: string): boolean {
  const target = rowElements(tree).find((row) => row.dataset.row === key);
  target?.focus();
  return target !== undefined;
}

export function focusCurrentRow(tree: HTMLElement | null): boolean {
  const rows = rowElements(tree);
  const target = rows.find((row) => row.tabIndex === 0) ?? rows[0];
  target?.focus();
  return target !== undefined;
}

export function rowKeyOf(target: EventTarget | null): string | null {
  if (!(target instanceof HTMLElement)) return null;
  return target.closest<HTMLElement>(ROW_SELECTOR)?.dataset.row ?? null;
}
