import { useState, type KeyboardEvent } from "react";
import { SEARCH_MAX_CHARS } from "../api/search";
import { Link, PATHS } from "../router";
import { SEARCH_HINT } from "../shell/globalKeys";
import { useKeyHints, type KeyHint } from "../shell/keyHints";
import { Kbd } from "../ui/Kbd";
import type { SidebarSearch } from "./useSidebarSearch";
import "./sidebar.css";

export const FIELD_HINTS: readonly KeyHint[] = [
  { keys: "⏎", label: "all results" },
  { keys: "esc", label: "clear" },
  { keys: "⌥↑ ⌥↓", label: "conversations" },
];

function FieldHints() {
  useKeyHints(FIELD_HINTS);
  return null;
}

function SearchField({ search }: { readonly search: SidebarSearch }) {
  const [focused, setFocused] = useState(false);
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    if (e.key === "Escape") {
      e.preventDefault();
      e.stopPropagation();
      search.clear();
    } else if (e.key === "Enter") {
      e.preventDefault();
      search.submit();
    }
  };
  return (
    <div className="th-nav-item th-nav-search" role="search" onClick={() => search.inputRef.current?.focus()}>
      <span className="th-nav-icon" aria-hidden="true">
        /
      </span>
      <input
        ref={search.inputRef}
        className="th-nav-search-input"
        type="text"
        role="searchbox"
        aria-label="search conversations"
        aria-keyshortcuts={SEARCH_HINT.keys}
        placeholder="search"
        autoComplete="off"
        spellCheck={false}
        enterKeyHint="search"
        maxLength={SEARCH_MAX_CHARS}
        value={search.query}
        onChange={(e) => search.setQuery(e.target.value)}
        onKeyDown={onKeyDown}
        onFocus={() => setFocused(true)}
        onBlur={() => {
          setFocused(false);
          search.leave();
        }}
      />
      <span aria-hidden="true">
        <Kbd>esc</Kbd>
      </span>
      {focused ? <FieldHints /> : null}
    </div>
  );
}

export interface SearchNavItemProps {
  readonly search: SidebarSearch;
  readonly current: boolean;
}

export function SearchNavItem({ search, current }: SearchNavItemProps) {
  if (search.open) return <SearchField search={search} />;
  return (
    <Link
      className="th-nav-item"
      to={PATHS.search}
      aria-current={current ? "page" : undefined}
      aria-keyshortcuts={SEARCH_HINT.keys}
      onClick={search.start}
    >
      <span className="th-nav-icon" aria-hidden="true">
        /
      </span>
      <span>search</span>
      <span aria-hidden="true">
        <Kbd>{SEARCH_HINT.keys}</Kbd>
      </span>
    </Link>
  );
}
