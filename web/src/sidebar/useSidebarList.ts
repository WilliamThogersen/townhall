import { useCallback, useMemo, useState } from "react";
import { useConversations, useTasks } from "../data";
import { useRoute } from "../router";
import { loadByRepo, saveByRepo } from "./byRepoPref";
import { normalizeQuery } from "./sections";
import { buildSections, repoBlocks, visibleIds, type GroupBlock, type RowSections } from "./sidebarRows";
import { useMessageMatches } from "./useMessageMatches";

export interface SidebarList {
  readonly sections: RowSections;
  readonly blocks: readonly GroupBlock[] | null;
  readonly order: readonly string[];
  readonly byRepo: boolean;
  readonly toggleByRepo: () => void;
  readonly toggleGroup: (key: string) => void;
  readonly searching: boolean;
  readonly total: number;
  readonly loading: boolean;
  readonly error: string | null;
  readonly matchError: string | null;
  readonly currentId: string | null;
}

function toggled(keys: ReadonlySet<string>, key: string): ReadonlySet<string> {
  return keys.has(key) ? new Set([...keys].filter((k) => k !== key)) : new Set([...keys, key]);
}

function useCurrentId(): string | null {
  const route = useRoute();
  return route.name === "conversation" ? (route.params.id ?? null) : null;
}

export function useSidebarList(query: string): SidebarList {
  const currentId = useCurrentId();
  const { conversations, loading, error } = useConversations();
  const { tasks } = useTasks();
  const matches = useMessageMatches(query);
  const [byRepo, setByRepo] = useState(loadByRepo);
  const [folded, setFolded] = useState<ReadonlySet<string>>(() => new Set());
  const searching = normalizeQuery(query) !== "";
  const sections = useMemo(
    () => buildSections(conversations, tasks, searching ? { query, textMatches: matches.ids } : undefined),
    [conversations, tasks, searching, query, matches.ids],
  );
  const blocks = useMemo(
    () => (byRepo ? repoBlocks(sections.active, { folded, currentId, searching }) : null),
    [byRepo, sections.active, folded, currentId, searching],
  );
  const order = useMemo(() => visibleIds(sections, blocks), [sections, blocks]);
  const toggleByRepo = useCallback(() => {
    saveByRepo(!byRepo);
    setByRepo(!byRepo);
  }, [byRepo]);
  const toggleGroup = useCallback((key: string) => setFolded((keys) => toggled(keys, key)), []);
  return {
    sections,
    blocks,
    order,
    byRepo,
    toggleByRepo,
    toggleGroup,
    searching,
    total: conversations.length,
    loading,
    error,
    matchError: searching ? matches.error : null,
    currentId,
  };
}
