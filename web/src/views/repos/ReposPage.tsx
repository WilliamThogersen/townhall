import { useCallback, useMemo, useState } from "react";
import { toMessage } from "../../api/errors";
import type { Repo } from "../../api/types";
import { useAgentInfos, useConversations, useRepos, useTasks } from "../../data";
import { NO_REPOS_TEXT } from "../../repos/copy";
import { userHomeOf } from "../../repos/repoPath";
import { summarizeRepos, type RepoSummary } from "../../repos/repoSummary";
import { useAddRepo, type AddRepoFlow } from "../../repos/useAddRepo";
import { useInfo } from "../../repos/useInfo";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { DRAG_REGION } from "../../ui/dragRegion";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { AddRepoForm } from "./AddRepoForm";
import { RepoRow } from "./RepoRow";
import { useRepoKeys } from "./useRepoKeys";
import "../settings/cap.css";

interface Problem {
  readonly message: string;
  readonly dismiss: (() => void) | null;
}

function useSummaries(repos: readonly Repo[] | null): readonly RepoSummary[] | null {
  const { conversations } = useConversations();
  const { tasks } = useTasks();
  const { agents } = useAgentInfos();
  const { info } = useInfo();
  const home = info ? userHomeOf(info.home) : null;
  return useMemo(
    () => (repos === null ? null : summarizeRepos(repos, { home, conversations, tasks, agents })),
    [repos, home, conversations, tasks, agents],
  );
}

function ReposHead({ count, flow }: { readonly count: number | null; readonly flow: AddRepoFlow }) {
  return (
    <header className="th-page-head" {...DRAG_REGION}>
      <h1>repos</h1>
      {count === null ? null : <span className="th-cap">{count}</span>}
      <div className="th-page-head-actions">
        <Button
          variant="primary"
          size="sm"
          busy={flow.desktop && flow.busy}
          busyLabel="adding…"
          aria-expanded={flow.desktop ? undefined : flow.formOpen}
          onClick={flow.begin}
        >
          add repository…
        </Button>
      </div>
    </header>
  );
}

function ProblemBanner({ problem }: { readonly problem: Problem | null }) {
  if (problem === null) return null;
  const action = problem.dismiss ? (
    <Button size="sm" onClick={problem.dismiss}>
      dismiss
    </Button>
  ) : undefined;
  return (
    <Banner level="error" action={action}>
      {problem.message}
    </Banner>
  );
}

interface BodyProps {
  readonly summaries: readonly RepoSummary[] | null;
  readonly loading: boolean;
  readonly flow: AddRepoFlow;
  readonly onRemove: (name: string) => Promise<void>;
}

function ReposBody({ summaries, loading, flow, onRemove }: BodyProps) {
  const form = flow.formOpen ? (
    <AddRepoForm busy={flow.busy} error={flow.error} focusToken={flow.focusToken} onSubmit={(input) => void flow.submit(input)} onCancel={flow.cancel} />
  ) : null;
  if (summaries === null) {
    return loading ? (
      <div className="th-empty">
        <Loading verb="loading repos" />
      </div>
    ) : null;
  }
  return (
    <>
      {form}
      {summaries.length === 0 && !flow.formOpen ? <EmptyState title="no repos yet">{NO_REPOS_TEXT}</EmptyState> : null}
      {summaries.map((summary) => (
        <RepoRow key={summary.repo.name} summary={summary} onRemove={onRemove} />
      ))}
    </>
  );
}

function useRemove(remove: (name: string) => Promise<void>) {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(
    async (name: string) => {
      setError(null);
      try {
        await remove(name);
      } catch (e) {
        setError(toMessage(e));
      }
    },
    [remove],
  );
  const dismiss = useCallback(() => setError(null), []);
  return { error, run, dismiss };
}

function pickProblem(removal: ReturnType<typeof useRemove>, flow: AddRepoFlow, loadError: string | null): Problem | null {
  if (removal.error !== null) return { message: removal.error, dismiss: removal.dismiss };
  if (flow.desktop && flow.error !== null) return { message: flow.error, dismiss: flow.dismissError };
  return loadError === null ? null : { message: `repos not loaded: ${loadError}`, dismiss: null };
}

export function ReposPage() {
  const { repos, loading, error, remove } = useRepos();
  const flow = useAddRepo();
  const removal = useRemove(remove);
  const summaries = useSummaries(repos);
  useRepoKeys(flow.formOpen, flow.begin);
  const problem = pickProblem(removal, flow, repos === null ? error : null);
  return (
    <div className="th-page">
      <ReposHead count={repos === null ? null : repos.length} flow={flow} />
      <div className="th-page-body">
        <ProblemBanner problem={problem} />
        <ReposBody summaries={summaries} loading={loading} flow={flow} onRemove={removal.run} />
      </div>
    </div>
  );
}
