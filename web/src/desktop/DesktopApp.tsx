import { App } from "../App";
import { Banner } from "../ui/Banner";
import { Button } from "../ui/Button";
import { DRAG_REGION } from "../ui/dragRegion";
import { EmptyState } from "../ui/EmptyState";
import { Loading } from "../ui/Loading";
import { useRepo, type RepoState } from "./useRepo";
import "./desktop.css";

/** Empty title bar row: first-run screens have no sidebar header to drag the window by. */
function DragBar() {
  return <div className="th-drag-bar" aria-hidden="true" {...DRAG_REGION} />;
}

function Starting() {
  return (
    <main className="th-main th-first-run">
      <DragBar />
      <div className="th-empty">
        <Loading verb="starting" />
      </div>
    </main>
  );
}

function RepoSetup({ repo }: { readonly repo: RepoState }) {
  const choose = (
    <Button variant="primary" busy={repo.busy} busyLabel="choosing…" onClick={() => void repo.choose()}>
      choose repository…
    </Button>
  );
  return (
    <main className="th-main th-first-run">
      <DragBar />
      {repo.error && <Banner level="error">{repo.error}</Banner>}
      <EmptyState logo title="choose the git repository your agents work in" action={choose} />
    </main>
  );
}

export function DesktopApp() {
  const repo = useRepo();
  const { status, error } = repo;

  if (!status && !error) return <Starting />;
  const ready = status !== null && status.mode !== "stopped";
  if (!ready) return <RepoSetup repo={repo} />;
  return <App />;
}
