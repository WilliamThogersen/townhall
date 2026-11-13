import { useTasks } from "../data";
import { Link, PATHS } from "../router";
import { overview } from "../tasks/progress";
import "./sidebar.css";

export function ProgressOverview({ bar = false }: { readonly bar?: boolean }) {
  const { tasks, loading } = useTasks();
  const { running, waiting, landed } = overview(tasks, Date.now());
  if (loading || running + waiting + landed === 0) return null;
  return (
    <Link className={bar ? "th-progress th-progress--bar" : "th-progress"} to={PATHS.control} role="group" aria-label="work in progress">
      <span className="th-progress-item" data-kind="running" hidden={running === 0}>{running} running</span>
      <span className="th-progress-item" data-kind="waiting" hidden={waiting === 0}>{waiting} needs you</span>
      <span className="th-progress-item" data-kind="landed" hidden={landed === 0}>{landed} landed</span>
    </Link>
  );
}
