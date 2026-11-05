import { useEffect, useState } from "react";
import { toMessage } from "../api/errors";
import { searchConversations } from "../api/search";

export const SEARCH_DELAY_MS = 200;

export interface MessageMatches {
  readonly ids: ReadonlySet<string>;
  readonly error: string | null;
}

interface Answer extends MessageMatches {
  readonly query: string;
}

const NOTHING: MessageMatches = { ids: new Set(), error: null };

const isAbort = (e: unknown) => e instanceof DOMException && e.name === "AbortError";

export function useMessageMatches(query: string, delayMs: number = SEARCH_DELAY_MS): MessageMatches {
  const text = query.trim();
  const [answer, setAnswer] = useState<Answer | null>(null);
  useEffect(() => {
    if (text === "") return;
    const abort = new AbortController();
    const timer = setTimeout(() => {
      searchConversations(text, abort.signal)
        .then((hits) => setAnswer({ query: text, ids: new Set(hits.map((h) => h.conversationId)), error: null }))
        .catch((e: unknown) => {
          if (!isAbort(e)) setAnswer({ query: text, ids: new Set(), error: toMessage(e) });
        });
    }, delayMs);
    return () => {
      clearTimeout(timer);
      abort.abort();
    };
  }, [text, delayMs]);
  return answer !== null && answer.query === text && text !== "" ? answer : NOTHING;
}
