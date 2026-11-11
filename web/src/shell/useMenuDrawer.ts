import { useCallback, useEffect, useState } from "react";
import { useRoute } from "../router";
import { isInTerminal, isTextField } from "./globalKeys";

export interface MenuDrawer {
  readonly open: boolean;
  readonly toggle: () => void;
}

function escapeIgnored(e: KeyboardEvent): boolean {
  if (e.defaultPrevented || e.isComposing || isInTerminal(e.target)) return true;
  if (e.metaKey || e.ctrlKey || e.altKey) return true;
  return isTextField(e.target) || (e.target instanceof Element && e.target.closest('[aria-modal="true"], dialog') !== null);
}

export function useMenuDrawer(): MenuDrawer {
  const [open, setOpen] = useState(false);
  const { path, query } = useRoute();
  const place = `${path}?${JSON.stringify(query)}`;
  const [seen, setSeen] = useState(place);
  if (seen !== place) {
    setSeen(place);
    setOpen(false);
  }
  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || escapeIgnored(e)) return;
      e.preventDefault();
      setOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [open]);
  const toggle = useCallback(() => setOpen((was) => !was), []);
  return { open, toggle };
}
