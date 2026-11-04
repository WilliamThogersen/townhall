import { useMemo } from "react";
import type { Repo } from "../api/types";
import { useRepos } from "../data";
import { navigate, PATHS } from "../router";
import { Loading } from "../ui/Loading";
import { Menu } from "../ui/Menu";
import { RowList, type RowHandlers } from "./ConversationRow";
import { RepoGroups } from "./RepoGroups";
import { useConversationKeys } from "./useConversationKeys";
import { useNow } from "./useNow";
import { useRowMenu } from "./useRowMenu";
import { useSidebarList, type SidebarList } from "./useSidebarList";

const NO_REPOS: readonly Repo[] = [];

interface NotesProps {
  readonly list: SidebarList;
  readonly query: string;
  readonly actionError: string | null;
}

function Note({ children, alert = false }: { readonly children: string; readonly alert?: boolean }) {
  return (
    <p className="th-sidebar-note" role={alert ? "alert" : undefined}>
      {children}
    </p>
  );
}

function Notes({ list, query, actionError }: NotesProps) {
  const shown = list.sections.active.length + list.sections.older.length;
  return (
    <>
      {list.loading && list.total === 0 ? (
        <p className="th-sidebar-note">
          <Loading verb="loading" />
        </p>
      ) : null}
      {list.error ? <Note alert>{`error: ${list.error}`}</Note> : null}
      {actionError ? <Note alert>{`error: ${actionError}`}</Note> : null}
      {list.matchError ? <Note>{`warn: message search failed: ${list.matchError}`}</Note> : null}
      {!list.loading && list.total === 0 ? <Note>no conversations yet</Note> : null}
      {list.searching && list.total > 0 && shown === 0 ? <Note>{`no conversations match ${query.trim()}`}</Note> : null}
    </>
  );
}

function ActiveSection({ list, handlers }: { readonly list: SidebarList; readonly handlers: RowHandlers }) {
  return (
    <>
      <h3 className="th-section">
        {`active ${list.sections.active.length}`}
        <button type="button" className="th-section-action" aria-pressed={list.byRepo} onClick={list.toggleByRepo}>
          by repo
        </button>
      </h3>
      {list.blocks ? (
        <RepoGroups blocks={list.blocks} onToggle={list.toggleGroup} handlers={handlers} />
      ) : (
        <RowList rows={list.sections.active} showRepo handlers={handlers} />
      )}
    </>
  );
}

export function SidebarConversations({ query }: { readonly query: string }) {
  const list = useSidebarList(query);
  const { repos } = useRepos();
  const now = useNow();
  const rows = useMemo(() => [...list.sections.active, ...list.sections.older], [list.sections]);
  const menu = useRowMenu(rows, repos ?? NO_REPOS);
  useConversationKeys(list.order, list.currentId);
  const handlers = useMemo<RowHandlers>(
    () => ({ currentId: list.currentId, now, onOpen: (id) => navigate(PATHS.conversation(id)), onMenu: menu.open }),
    [list.currentId, now, menu.open],
  );
  return (
    <div className="th-sidebar-scroll">
      <ActiveSection list={list} handlers={handlers} />
      <Notes list={list} query={query} actionError={menu.error} />
      <h3 className="th-section">{`older ${list.sections.older.length}`}</h3>
      <RowList rows={list.sections.older} showRepo handlers={handlers} />
      {menu.at ? <Menu items={menu.items} label="conversation actions" onClose={menu.close} position={menu.at} /> : null}
    </div>
  );
}
