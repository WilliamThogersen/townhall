import { isInTerminal, isTextField } from "../../../shell/globalKeys";

export function isPlainKey(e: KeyboardEvent): boolean {
  if (e.defaultPrevented || e.isComposing) return false;
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  if (isTextField(e.target)) return false;
  return !(e.target instanceof Element && e.target.closest("dialog") !== null);
}

export { isInTerminal };
