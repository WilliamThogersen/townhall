import type { PutRepoAccountsInput, Repo } from "../../api/types";
import type { ForgeAccount, ForgeKind } from "../../settings/settings";
import { Segmented, type SegmentedOption } from "../../ui/Segmented";

const AUTO = "";
const FORGES: readonly { readonly forge: ForgeKind; readonly title: string }[] = [
  { forge: "github", title: "GitHub" },
  { forge: "gitlab", title: "GitLab" },
];

export type PickAccount = (name: string, input: PutRepoAccountsInput) => Promise<void>;

function pickOf(repo: Repo, forge: ForgeKind): string | null {
  return forge === "github" ? repo.githubAccount : repo.gitlabAccount;
}

function inputFor(forge: ForgeKind, login: string | null): PutRepoAccountsInput {
  return forge === "github" ? { githubAccount: login } : { gitlabAccount: login };
}

export function accountOptions(
  accounts: readonly ForgeAccount[],
  forge: ForgeKind,
  picked: string | null,
): readonly SegmentedOption<string>[] {
  const ofForge = accounts.filter((account) => account.forge === forge);
  if (ofForge.length === 0) return [];
  const logins = new Map<string, string>();
  ofForge.forEach((account) => {
    if (!logins.has(account.login)) logins.set(account.login, account.label);
  });
  const registered = [...logins].map(([value, label]) => ({ value, label }));
  const stale = picked !== null && !logins.has(picked) ? [{ value: picked, label: `${picked} (not registered)` }] : [];
  return [{ value: AUTO, label: "auto" }, ...registered, ...stale];
}

interface Props {
  readonly repo: Repo;
  readonly accounts: readonly ForgeAccount[];
  readonly onPick: PickAccount;
}

export function RepoAccounts({ repo, accounts, onPick }: Props) {
  return (
    <div className="th-repo-accounts">
      {FORGES.map(({ forge, title }) => {
        const picked = pickOf(repo, forge);
        const options = accountOptions(accounts, forge, picked);
        if (options.length === 0) return null;
        return (
          <div key={forge} className="th-repo-account">
            <span className="th-cap">{`${forge} account`}</span>
            <Segmented
              name={`th-repo-account-${repo.name}-${forge}`}
              label={`${title} account for ${repo.name}`}
              value={picked ?? AUTO}
              options={options}
              onChange={(login) => void onPick(repo.name, inputFor(forge, login === AUTO ? null : login))}
            />
          </div>
        );
      })}
    </div>
  );
}
