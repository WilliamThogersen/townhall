import type { RepoState } from "./useRepo";

/** Header control: shows the current repo and lets the user change it. */
export function RepoSettings({ repo }: { repo: RepoState }) {
  const { status, busy, error, choose } = repo;
  const label =
    status?.mode === "external"
      ? "existing daemon on :7777"
      : (status?.repo ?? "no repository");
  return (
    <div className="repo-settings">
      <span className="muted" title={status?.repo ?? undefined}>{label}</span>
      <button type="button" onClick={() => void choose()} disabled={busy || status?.mode === "external"}>
        {busy ? "Working..." : "Change repo"}
      </button>
      {error && <span className="error" role="alert">{error}</span>}
    </div>
  );
}
