import { useCallback, useEffect, useRef, useState } from "react";
import { focusAgentRow, focusCurrentRow } from "./rowFocus";

interface FocusRequest {
  readonly name: string | null;
}

export function useRowFocus() {
  const listRef = useRef<HTMLUListElement>(null);
  const fallbackRef = useRef<HTMLButtonElement>(null);
  const [request, setRequest] = useState<FocusRequest | null>(null);

  const focusRow = useCallback((name: string | null) => {
    const list = listRef.current;
    if (name !== null && focusAgentRow(list, name)) return;
    if (!focusCurrentRow(list)) fallbackRef.current?.focus();
  }, []);

  const focusRowAfterRender = useCallback((name: string | null) => setRequest({ name }), []);

  useEffect(() => {
    if (request === null) return;
    focusRow(request.name);
    setRequest(null);
  }, [request, focusRow]);

  return { listRef, fallbackRef, focusRow, focusRowAfterRender };
}
