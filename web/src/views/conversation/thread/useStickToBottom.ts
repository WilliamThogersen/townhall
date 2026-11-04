import { useCallback, useEffect, useLayoutEffect, useRef } from "react";

export const PIN_SLACK_PX = 32;

export interface ScrollBox {
  readonly scrollHeight: number;
  readonly scrollTop: number;
  readonly clientHeight: number;
}

export function isAtBottom(box: ScrollBox): boolean {
  return box.scrollHeight - box.scrollTop - box.clientHeight <= PIN_SLACK_PX;
}

export function useStickToBottom(content: unknown, tail: unknown) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const contentRef = useRef<HTMLDivElement>(null);
  const pinned = useRef(true);

  const stick = useCallback(() => {
    const box = scrollRef.current;
    if (box !== null && pinned.current) box.scrollTop = box.scrollHeight;
  }, []);

  const onScroll = useCallback(() => {
    const box = scrollRef.current;
    if (box !== null) pinned.current = isAtBottom(box);
  }, []);

  useLayoutEffect(stick, [content, tail, stick]);

  useEffect(() => {
    const inner = contentRef.current;
    if (inner === null || typeof ResizeObserver === "undefined") return;
    const observer = new ResizeObserver(stick);
    observer.observe(inner);
    return () => observer.disconnect();
  }, [stick]);

  return { scrollRef, contentRef, onScroll };
}
