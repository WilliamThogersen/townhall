import { useCallback, useMemo, useState } from "react";
import { toMessage } from "../api/errors";
import type { Repo } from "../api/types";
import { useConversationActions } from "../data";
import type { MenuItem } from "../ui/Menu";
import type { MenuPoint } from "./ConversationRow";
import { rowMenuItems } from "./rowMenuItems";
import type { SidebarRow } from "./sidebarRows";

interface OpenMenu {
  readonly id: string;
  readonly at: MenuPoint;
  readonly from: HTMLElement;
}

export interface RowMenuState {
  readonly open: (id: string, at: MenuPoint, from: HTMLElement) => void;
  readonly close: () => void;
  readonly at: MenuPoint | null;
  readonly items: readonly MenuItem[];
  readonly error: string | null;
}

export function useRowMenu(rows: readonly SidebarRow[], repos: readonly Repo[]): RowMenuState {
  const actions = useConversationActions();
  const [menu, setMenu] = useState<OpenMenu | null>(null);
  const [error, setError] = useState<string | null>(null);
  const run = useCallback((work: Promise<unknown>) => {
    setError(null);
    work.catch((e: unknown) => setError(toMessage(e)));
  }, []);
  const open = useCallback((id: string, at: MenuPoint, from: HTMLElement) => setMenu({ id, at, from }), []);
  const close = useCallback(() => {
    menu?.from.focus();
    setMenu(null);
  }, [menu]);
  const row = menu === null ? undefined : rows.find((r) => r.conversation.id === menu.id);
  const items = useMemo(() => {
    if (!row) return [];
    return rowMenuItems(row, repos, {
      end: (id) => run(actions.end(id)),
      link: (id, repo) => run(actions.patch(id, { linkedRepo: repo })),
    });
  }, [row, repos, actions, run]);
  return { open, close, at: row && menu ? menu.at : null, items, error };
}
