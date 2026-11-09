import { useCallback, useMemo, useState } from "react";
import { toMessage } from "../../api/errors";
import type { PatchRepoInput, PutRepoAccountsInput, Repo } from "../../api/types";
import { useAgentInfos, useConversations, useRepos, useTasks } from "../../data";
import { NO_REPOS_TEXT } from "../../repos/copy";
import { clonesDirOf, userHomeOf } from "../../repos/repoPath";
import { summarizeRepos, type RepoSummary } from "../../repos/repoSummary";
import { useAddRepo, type AddRepoFlow } from "../../repos/useAddRepo";
import { useInfo } from "../../repos/useInfo";
import type { ForgeAccount } from "../../settings/settings";
import { useSettings } from "../../settings/useSettings";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { DRAG_REGION } from "../../ui/dragRegion";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { AddRepoModal } from "./AddRepoModal";
import type { PickAccount } from "./RepoAccounts";
import { RepoRow } from "./RepoRow";
import { useRepoKeys } from "./useRepoKeys";
import "../settings/cap.css";

type RemoveRepo = (name: string, deleteClone?: boolean) => Promise<void>;
type SetAccounts = (name: string, input: PutRepoAccountsInput) => Promise<Repo>;
type EditRepo = (name: string, input: PatchRepoInput) => Promise<Repo>;

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
  const clonesDir = info ? clonesDirOf(info.home) : null;
  return useMemo(
    () => (repos === null ? null : summarizeRepos(repos, { home, clonesDir, conversations, tasks, agents })),
    [repos, home, clonesDir, conversations, tasks, agents],
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
          aria-expanded={flow.formOpen}
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
  readonly onRemove: RemoveRepo;
  readonly onPickAccount: PickAccount;
  readonly onEdit: EditRepo;
  readonly accounts: readonly ForgeAccount[];
  readonly defaultCloneDir: string;
}

function ReposBody({ summaries, loading, flow, onRemove, onPickAccount, onEdit, accounts, defaultCloneDir }: BodyProps) {
  const form = flow.formOpen ? (
    <AddRepoModal
      busy={flow.busy}
      error={flow.error}
      focusToken={flow.focusToken}
      defaultCloneDir={defaultCloneDir}
      onBrowse={flow.desktop ? flow.browse : undefined}
      onSubmit={(input) => void flow.submit(input)}
      onCancel={flow.cancel}
    />
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
        <RepoRow key={summary.repo.name} summary={summary} onRemove={onRemove} accounts={accounts} onPickAccount={onPickAccount} onEdit={onEdit} />
      ))}
    </>
  );
}

function useRemove(remove: RemoveRepo, setAccounts: SetAccounts) {
  const [error, setError] = useState<string | null>(null);
  const run = useCallback<RemoveRepo>(
    async (name, deleteClone) => {
      setError(null);
      try {
        await remove(name, deleteClone);
      } catch (e) {
        setError(toMessage(e));
      }
    },
    [remove],
  );
  const pick = useCallback<PickAccount>(
    async (name, input) => {
      setError(null);
      try {
        await setAccounts(name, input);
      } catch (e) {
        setError(toMessage(e));
      }
    },
    [setAccounts],
  );
  const dismiss = useCallback(() => setError(null), []);
  return { error, run, pick, dismiss };
}

function pickProblem(removal: ReturnType<typeof useRemove>, loadError: string | null): Problem | null {
  if (removal.error !== null) return { message: removal.error, dismiss: removal.dismiss };
  return loadError === null ? null : { message: `repos not loaded: ${loadError}`, dismiss: null };
}

export function ReposPage() {
  const { repos, loading, error, remove, setAccounts, edit } = useRepos();
  const { settings } = useSettings();
  const flow = useAddRepo();
  const removal = useRemove(remove, setAccounts);
  const summaries = useSummaries(repos);
  useRepoKeys(flow.formOpen, flow.begin);
  const problem = pickProblem(removal, repos === null ? error : null);
  return (
    <div className="th-page">
      <ReposHead count={repos === null ? null : repos.length} flow={flow} />
      <div className="th-page-body">
        <ProblemBanner problem={problem} />
        <ReposBody
          summaries={summaries}
          loading={loading}
          flow={flow}
          onRemove={removal.run}
          onPickAccount={removal.pick}
          onEdit={edit}
          accounts={settings.forgeAccounts}
          defaultCloneDir={settings.clonesDir}
        />
      </div>
    </div>
  );
}
