import { useSyncExternalStore } from "react";

function subscribe(onChange: () => void): () => void {
  document.addEventListener("visibilitychange", onChange);
  return () => document.removeEventListener("visibilitychange", onChange);
}

const isVisible = () => document.visibilityState !== "hidden";

export function useDocumentVisible(): boolean {
  return useSyncExternalStore(subscribe, isVisible, () => true);
}
