import { NEW_CONVERSATION_HINT } from "../../shell/globalKeys";
import { useKeyHints } from "../../shell/keyHints";
import { EmptyState } from "../../ui/EmptyState";

const SEARCH_KEYS = [NEW_CONVERSATION_HINT];

export function SearchPage() {
  useKeyHints(SEARCH_KEYS);
  return (
    <EmptyState title="search comes next">
      there are no conversations to search yet
    </EmptyState>
  );
}
