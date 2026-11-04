import { Link, PATHS } from "../../router";
import { EmptyState } from "../../ui/EmptyState";

export function FirstRunSlot() {
  const add = (
    <Link className="th-btn th-btn--primary" to={PATHS.repos}>
      add repository…
    </Link>
  );
  return (
    <EmptyState logo title="add your first repository" action={add}>
      townhall works in git worktrees of your repos. it never writes into a repo itself.
    </EmptyState>
  );
}
