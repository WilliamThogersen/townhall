import { useRef, type RefObject } from "react";
import { Button } from "../../ui/Button";
import { Menu, type MenuItem } from "../../ui/Menu";

export const ALL_REPOS = "all repos";

export interface RepoFilterProps {
  readonly repos: readonly string[];
  readonly value: string | null;
  readonly open: boolean;
  readonly buttonRef: RefObject<HTMLButtonElement | null>;
  readonly onOpenChange: (open: boolean) => void;
  readonly onPick: (repo: string | null) => void;
}

const sameRepo = (a: string | null, b: string | null) => a?.toLowerCase() === b?.toLowerCase();

export function repoChoices(repos: readonly string[], value: string | null): readonly string[] {
  const known = repos.some((repo) => sameRepo(repo, value));
  return value === null || known ? repos : [...repos, value];
}

function menuItems(repos: readonly string[], value: string | null, onPick: (repo: string | null) => void): MenuItem[] {
  const all: MenuItem = { id: "all", label: ALL_REPOS, role: "menuitemradio", checked: value === null, onSelect: () => onPick(null) };
  const each = repoChoices(repos, value).map<MenuItem>((repo) => ({
    id: `repo:${repo}`,
    label: repo,
    role: "menuitemradio",
    checked: sameRepo(repo, value),
    onSelect: () => onPick(repo),
  }));
  return [all, ...each];
}

export function RepoFilter({ repos, value, open, buttonRef, onOpenChange, onPick }: RepoFilterProps) {
  const pressedWhileOpen = useRef(false);
  const close = () => {
    onOpenChange(false);
    buttonRef.current?.focus();
  };
  const press = () => {
    if (!pressedWhileOpen.current) onOpenChange(true);
    pressedWhileOpen.current = false;
  };
  return (
    <div className="th-control-filter">
      <Button
        ref={buttonRef}
        variant="ghost"
        size="sm"
        aria-haspopup="menu"
        aria-expanded={open}
        onPointerDown={() => {
          pressedWhileOpen.current = open;
        }}
        onClick={press}
      >
        {value ?? ALL_REPOS} ▾
      </Button>
      {open ? <Menu items={menuItems(repos, value, onPick)} label="filter by repo" onClose={close} /> : null}
    </div>
  );
}
