import { Link, PATHS } from "../../router";
import { NEW_CONVERSATION_HINT, SEARCH_HINT } from "../../shell/globalKeys";
import { useKeyHints } from "../../shell/keyHints";
import { EmptyState } from "../../ui/EmptyState";

const CONVERSATION_KEYS = [NEW_CONVERSATION_HINT, SEARCH_HINT];

export function ConversationPage({ id }: { readonly id: string }) {
  useKeyHints(CONVERSATION_KEYS);
  const open = <Link className="th-btn" to={PATHS.control}>open control center</Link>;
  return (
    <EmptyState title="conversations come next" action={open}>
      conversation {id} will open here
    </EmptyState>
  );
}
