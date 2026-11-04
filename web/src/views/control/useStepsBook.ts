import { useCallback, useState } from "react";
import type { StepsBook, StepsEntry } from "./traceRows";

const EMPTY_BOOK: StepsBook = new Map();

export interface StepsBookState {
  readonly book: StepsBook;
  readonly record: (key: string, entry: StepsEntry) => void;
}

export function useStepsBook(): StepsBookState {
  const [book, setBook] = useState<StepsBook>(EMPTY_BOOK);
  const record = useCallback(
    (key: string, entry: StepsEntry) => setBook((prev) => new Map(prev).set(key, entry)),
    [],
  );
  return { book, record };
}
