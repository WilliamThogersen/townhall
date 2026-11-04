import { useEffect, useState } from "react";
import { toMessage } from "../../api/errors";
import { searchConversations, type SearchHit } from "../../api/search";

export interface SearchHitsState {
  readonly hits: readonly SearchHit[];
  readonly loading: boolean;
  readonly error: string | null;
}

interface Answer {
  readonly query: string;
  readonly hits: readonly SearchHit[];
  readonly error: string | null;
}

const NO_HITS: readonly SearchHit[] = [];

const isAbort = (e: unknown) => e instanceof DOMException && e.name === "AbortError";

export function useSearchHits(query: string): SearchHitsState {
  const text = query.trim();
  const [answer, setAnswer] = useState<Answer | null>(null);
  useEffect(() => {
    if (text === "") return;
    const abort = new AbortController();
    searchConversations(text, abort.signal)
      .then((hits) => setAnswer({ query: text, hits, error: null }))
      .catch((e: unknown) => {
        if (!isAbort(e)) setAnswer({ query: text, hits: NO_HITS, error: toMessage(e) });
      });
    return () => abort.abort();
  }, [text]);
  const current = answer !== null && answer.query === text ? answer : null;
  return {
    hits: current?.hits ?? NO_HITS,
    loading: text !== "" && current === null,
    error: current?.error ?? null,
  };
}
