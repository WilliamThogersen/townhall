import { useCallback, useEffect, useRef, useState, type RefObject } from "react";
import { isTextField, SEARCH_HINT } from "../shell/globalKeys";
import { navigate, PATHS } from "../router";
import { insideOverlay } from "./useConversationKeys";

export const NARROW_QUERY = "(max-width: 639px)";

export interface SidebarSearch {
  readonly query: string;
  readonly open: boolean;
  readonly inputRef: RefObject<HTMLInputElement | null>;
  readonly setQuery: (query: string) => void;
  readonly start: () => void;
  readonly clear: () => void;
  readonly leave: () => void;
  readonly submit: () => void;
}

export const searchPath = (query: string) => `${PATHS.search}?q=${encodeURIComponent(query.trim())}`;

function sidebarHidden(): boolean {
  return typeof window.matchMedia === "function" && window.matchMedia(NARROW_QUERY).matches;
}

function isSlash(e: KeyboardEvent): boolean {
  if (e.key !== SEARCH_HINT.keys || e.defaultPrevented || e.isComposing) return false;
  if (e.metaKey || e.ctrlKey || e.altKey) return false;
  return !isTextField(e.target) && !insideOverlay(e.target);
}

function useSlashKey(start: () => void): void {
  const latest = useRef(start);
  useEffect(() => {
    latest.current = start;
  });
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (!isSlash(e) || sidebarHidden()) return;
      e.preventDefault();
      latest.current();
    };
    window.addEventListener("keydown", onKeyDown, true);
    return () => window.removeEventListener("keydown", onKeyDown, true);
  }, []);
}

export function useSidebarSearch(): SidebarSearch {
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const focusNext = useRef(false);
  useEffect(() => {
    if (!open || !focusNext.current) return;
    focusNext.current = false;
    inputRef.current?.focus();
  }, [open]);
  const start = useCallback(() => {
    if (inputRef.current) inputRef.current.focus();
    else focusNext.current = true;
    setOpen(true);
  }, []);
  const clear = useCallback(() => {
    setQuery("");
    setOpen(false);
    inputRef.current?.blur();
  }, []);
  const leave = useCallback(() => {
    if (query.trim() === "") clear();
  }, [query, clear]);
  const submit = useCallback(() => {
    if (query.trim() === "") return;
    navigate(searchPath(query));
    inputRef.current?.blur();
  }, [query]);
  useSlashKey(start);
  return { query, open, inputRef, setQuery, start, clear, leave, submit };
}
