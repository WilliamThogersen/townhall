import { useCallback, useMemo } from "react";
import { navigate, PATHS, useRoute } from "../../router";
import type { ControlFilter } from "./traceScope";

export const CONVERSATION_PARAM = "c";
export const REPO_PARAM = "repo";

export function controlHref(filter: ControlFilter): string {
  const query = new URLSearchParams();
  if (filter.conversationId) query.set(CONVERSATION_PARAM, filter.conversationId);
  if (filter.repo) query.set(REPO_PARAM, filter.repo);
  const search = query.toString();
  return search ? `${PATHS.control}?${search}` : PATHS.control;
}

export interface ControlFilterState {
  readonly filter: ControlFilter;
  readonly setRepo: (repo: string | null) => void;
}

export function useControlFilter(): ControlFilterState {
  const { query } = useRoute();
  const conversationId = query[CONVERSATION_PARAM] || null;
  const repo = query[REPO_PARAM] || null;
  const filter = useMemo(() => ({ conversationId, repo }), [conversationId, repo]);
  const setRepo = useCallback(
    (next: string | null) => navigate(controlHref({ conversationId, repo: next }), { replace: true }),
    [conversationId],
  );
  return { filter, setRepo };
}
