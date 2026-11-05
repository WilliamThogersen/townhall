import type { Repo } from "../api/types";
import type { MenuItem } from "../ui/Menu";
import { repoLabel } from "./conversationRepo";
import type { SidebarRow } from "./sidebarRows";

export interface RowMenuActions {
  readonly end: (id: string) => void;
  readonly link: (id: string, repo: string | null) => void;
}

export function rowMenuItems(row: SidebarRow, repos: readonly Repo[], actions: RowMenuActions): readonly MenuItem[] {
  const { id, ended, linkedRepo } = row.conversation;
  const linked = linkedRepo?.toLowerCase() ?? null;
  const choose = (repo: string | null) => () => {
    if ((repo?.toLowerCase() ?? null) !== linked) actions.link(id, repo);
  };
  const end: readonly MenuItem[] = ended ? [] : [{ id: "end", label: "end conversation", onSelect: () => actions.end(id) }];
  const auto: MenuItem = {
    id: "auto",
    label: "repo: auto",
    role: "menuitemradio",
    checked: linked === null,
    detail: repoLabel(row.autoRepo),
    onSelect: choose(null),
  };
  const named = repos.map<MenuItem>((repo) => ({
    id: `repo:${repo.name}`,
    label: `repo: ${repo.name}`,
    role: "menuitemradio",
    checked: linked === repo.name.toLowerCase(),
    detail: repo.baseBranch,
    onSelect: choose(repo.name),
  }));
  return [...end, auto, ...named];
}
