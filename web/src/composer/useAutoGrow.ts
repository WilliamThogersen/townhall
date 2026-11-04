import { useLayoutEffect, type RefObject } from "react";

export function supportsFieldSizing(): boolean {
  return typeof CSS !== "undefined" && typeof CSS?.supports === "function" && CSS.supports("field-sizing", "content");
}

export function useAutoGrow(ref: RefObject<HTMLTextAreaElement | null>, value: string): void {
  useLayoutEffect(() => {
    const area = ref.current;
    if (area === null || supportsFieldSizing()) return;
    area.style.height = "auto";
    area.style.height = `${area.scrollHeight}px`;
  }, [ref, value]);
}
