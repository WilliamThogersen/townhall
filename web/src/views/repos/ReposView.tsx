import type { ReactNode } from "react";
import { useAgentsContext } from "../../agents/AgentsProvider";
import type { InfoState } from "../../repos/useInfo";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { DRAG_REGION } from "../../ui/dragRegion";
import { Loading } from "../../ui/Loading";
import { RepoRow } from "./RepoRow";
import "../settings/cap.css";
import "./repos.css";

export interface ReposViewProps {
  readonly repo: InfoState;
  readonly action: ReactNode;
  readonly problem?: string | null;
}

function Problem({ repo, problem }: Pick<ReposViewProps, "repo" | "problem">) {
  if (repo.error) {
    const retry = (
      <Button size="sm" busy={repo.loading} busyLabel="retrying…" onClick={repo.reload}>
        retry
      </Button>
    );
    return (
      <Banner level="error" action={retry}>
        repo not loaded: {repo.error}
      </Banner>
    );
  }
  return problem ? <Banner level="error">{problem}</Banner> : null;
}

export function ReposView({ repo, action, problem }: ReposViewProps) {
  const { agents } = useAgentsContext();
  const { info } = repo;
  return (
    <div className="th-page">
      <header className="th-page-head" {...DRAG_REGION}>
        <h1>repos</h1>
        {info ? <span className="th-cap">1</span> : null}
        {action}
      </header>
      <div className="th-page-body">
        <Problem repo={repo} problem={problem} />
        {info ? <RepoRow info={info} agents={agents} /> : null}
        {!info && repo.loading ? (
          <div className="th-empty">
            <Loading verb="loading repos" />
          </div>
        ) : null}
      </div>
    </div>
  );
}
