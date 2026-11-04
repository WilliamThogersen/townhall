import type { SearchHit } from "../../api/search";
import { Link, PATHS } from "../../router";
import { ageLabel } from "../../sidebar/age";
import { markParts } from "./markParts";

function Marked({ text, query }: { readonly text: string; readonly query: string }) {
  return markParts(text, query).map((part, i) =>
    part.marked ? <mark key={i}>{part.text}</mark> : <span key={i}>{part.text}</span>,
  );
}

function HitRow({ hit, query, now }: { readonly hit: SearchHit; readonly query: string; readonly now: number }) {
  return (
    <li>
      <Link className="th-hit" to={PATHS.conversation(hit.conversationId)}>
        <span className="th-hit-title" title={hit.title}>
          {hit.match === "title" ? <Marked text={hit.title} query={query} /> : hit.title}
        </span>
        <span className="th-hit-time">{ageLabel(hit.updatedAt, now)}</span>
        <span className="th-hit-text">
          {hit.match === "title" ? "title" : <Marked text={hit.snippet} query={query} />}
        </span>
      </Link>
    </li>
  );
}

export interface HitListProps {
  readonly hits: readonly SearchHit[];
  readonly query: string;
  readonly now: number;
}

export function HitList({ hits, query, now }: HitListProps) {
  return (
    <ul className="th-list" aria-label="search results">
      {hits.map((hit) => (
        <HitRow key={hit.conversationId} hit={hit} query={query} now={now} />
      ))}
    </ul>
  );
}
