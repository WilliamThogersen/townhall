import { Button } from "../ui/Button";
import type { RepoStatus } from "./bridge";
import type { RepoState } from "./useRepo";
import "./desktop.css";

export function repoLabel(status: RepoStatus | null): string {
  if (status?.mode === "external") return "daemon on :7777";
  if (!status?.repo) return "no repository";
  return status.repo.split("/").filter(Boolean).at(-1) ?? status.repo;
}

export function RepoSettings({ repo }: { readonly repo: RepoState }) {
  const { status, busy, error, choose } = repo;
  return (
    <div className="th-repo-control">
      {error ? (
        <span className="th-repo-control-label" role="alert" title={error}>
          <span className="th-level">error:</span> {error}
        </span>
      ) : (
        <span className="th-repo-control-label" title={status?.repo ?? undefined}>
          {repoLabel(status)}
        </span>
      )}
      <Button
        variant="ghost"
        size="sm"
        busy={busy}
        busyLabel="choosing…"
        disabled={status?.mode === "external"}
        onClick={() => void choose()}
      >
        change repo…
      </Button>
    </div>
  );
}
