import { Link, PATHS } from "../router";
import { NEW_CONVERSATION_HINT, SEARCH_HINT } from "../shell/globalKeys";
import { useKeyHints } from "../shell/keyHints";
import { EmptyState } from "../ui/EmptyState";

const BOTH_KEYS = [NEW_CONVERSATION_HINT, SEARCH_HINT];

function OpenControlCenter() {
  return <Link className="th-btn" to={PATHS.control}>open control center</Link>;
}

export function HomePlaceholder() {
  useKeyHints([SEARCH_HINT]);
  return (
    <EmptyState title="conversations come next" action={<OpenControlCenter />}>
      start agents from the control center for now
    </EmptyState>
  );
}

export function ConversationPlaceholder({ id }: { readonly id: string }) {
  useKeyHints(BOTH_KEYS);
  return (
    <EmptyState title="conversations come next" action={<OpenControlCenter />}>
      conversation {id} will open here
    </EmptyState>
  );
}

export function SearchPlaceholder() {
  useKeyHints([NEW_CONVERSATION_HINT]);
  return (
    <EmptyState title="search comes next">
      there are no conversations to search yet
    </EmptyState>
  );
}

export function NotFound({ path }: { readonly path: string }) {
  useKeyHints(BOTH_KEYS);
  const home = <Link className="th-btn" to={PATHS.home}>go home</Link>;
  return (
    <EmptyState title="not found" action={home}>
      nothing lives at {path}
    </EmptyState>
  );
}
