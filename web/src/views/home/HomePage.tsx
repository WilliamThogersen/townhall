import { Link, PATHS } from "../../router";
import { SEARCH_HINT } from "../../shell/globalKeys";
import { useKeyHints } from "../../shell/keyHints";
import { EmptyState } from "../../ui/EmptyState";

const HOME_KEYS = [SEARCH_HINT];

export function HomePage() {
  useKeyHints(HOME_KEYS);
  const open = <Link className="th-btn" to={PATHS.control}>open control center</Link>;
  return (
    <EmptyState title="conversations come next" action={open}>
      start agents from the control center for now
    </EmptyState>
  );
}
