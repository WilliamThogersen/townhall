import { useMemo, useSyncExternalStore } from "react";
import { currentUrl, subscribe } from "./history";
import { matchRoute, type Route } from "./match";

export function useRoute(): Route {
  const url = useSyncExternalStore(subscribe, currentUrl);
  return useMemo(() => {
    const parsed = new URL(url, "http://townhall.local");
    return matchRoute(parsed.pathname, parsed.search);
  }, [url]);
}
