import { useCallback } from "react";
import { navigate, PATHS, useRoute } from "../../router";

export const OPEN_AGENT_PARAM = "agent";

export interface OpenAgent {
  readonly openName: string | null;
  readonly open: (name: string) => void;
  readonly close: () => void;
}

export function controlPath(name: string | null = null): string {
  if (!name) return PATHS.control;
  return `${PATHS.control}?${OPEN_AGENT_PARAM}=${encodeURIComponent(name)}`;
}

export function useOpenAgent(): OpenAgent {
  const route = useRoute();
  const openName = route.query[OPEN_AGENT_PARAM] || null;
  const open = useCallback((name: string) => navigate(controlPath(name), { replace: true }), []);
  const close = useCallback(() => navigate(controlPath(), { replace: true }), []);
  return { openName, open, close };
}
