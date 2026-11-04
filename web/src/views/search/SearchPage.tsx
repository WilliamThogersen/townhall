import { useRoute } from "../../router";
import { NEW_CONVERSATION_HINT, SEARCH_HINT } from "../../shell/globalKeys";
import { useKeyHints } from "../../shell/keyHints";
import { useNow } from "../../sidebar/useNow";
import { Banner } from "../../ui/Banner";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { HitList } from "./HitList";
import { useSearchHits, type SearchHitsState } from "./useSearchHits";
import "../settings/cap.css";
import "./search.css";

const SEARCH_KEYS = [NEW_CONVERSATION_HINT, SEARCH_HINT];

export const countLabel = (n: number) => `${n} conversation${n === 1 ? "" : "s"}`;

function SearchBody({ query, state }: { readonly query: string; readonly state: SearchHitsState }) {
  const now = useNow();
  if (query === "") {
    return <EmptyState title="search conversations">press / to search titles and message text</EmptyState>;
  }
  if (state.error !== null) return <Banner level="error">{state.error}</Banner>;
  if (state.loading) {
    return (
      <div className="th-empty">
        <Loading verb="searching" />
      </div>
    );
  }
  if (state.hits.length === 0) return <EmptyState title={`no conversations match ${query}`} />;
  return <HitList hits={state.hits} query={query} now={now} />;
}

export function SearchPage() {
  useKeyHints(SEARCH_KEYS);
  const query = (useRoute().query.q ?? "").trim();
  const state = useSearchHits(query);
  const done = query !== "" && !state.loading && state.error === null;
  return (
    <div className="th-page">
      <header className="th-page-head">
        <h1>search</h1>
        {query !== "" ? (
          <span className="th-cap th-search-cap" title={query}>
            {done ? `${query} · ${countLabel(state.hits.length)}` : query}
          </span>
        ) : null}
      </header>
      <div className="th-page-body">
        <SearchBody query={query} state={state} />
      </div>
    </div>
  );
}
