import { Link, PATHS } from "../router";
import { NEW_CONVERSATION_HINT, SEARCH_HINT } from "../shell/globalKeys";
import { useKeyHints } from "../shell/keyHints";
import { EmptyState } from "../ui/EmptyState";

const NOT_FOUND_KEYS = [NEW_CONVERSATION_HINT, SEARCH_HINT];

export function NotFound({ path }: { readonly path: string }) {
  useKeyHints(NOT_FOUND_KEYS);
  const home = <Link className="th-btn" to={PATHS.home}>go home</Link>;
  return (
    <EmptyState title="not found" action={home}>
      nothing lives at {path}
    </EmptyState>
  );
}
