import { App } from "../App";
import { RepoSettings } from "./RepoSettings";
import { useRepo } from "./useRepo";

/** Desktop shell: makes sure a repo is chosen and the daemon is up, then shows the app. */
export function DesktopApp() {
  const repo = useRepo();
  const { status, busy, error, choose } = repo;

  if (!status && !error) return <p className="muted">Starting...</p>;
  const ready = status !== null && status.mode !== "stopped";
  if (!ready) {
    return (
      <div className="app">
        <h1>townhall</h1>
        <p>Choose the git repository your agents will work in.</p>
        {error && <p className="banner error" role="alert">{error}</p>}
        <button type="button" onClick={() => void choose()} disabled={busy}>
          {busy ? "Working..." : "Choose repository..."}
        </button>
      </div>
    );
  }
  return <App toolbar={<RepoSettings repo={repo} />} />;
}
