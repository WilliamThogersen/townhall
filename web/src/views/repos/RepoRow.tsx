import { useState } from "react";
import type { PatchRepoInput } from "../../api/types";
import type { RepoSummary } from "../../repos/repoSummary";
import type { ForgeAccount } from "../../settings/settings";
import { Link, PATHS } from "../../router";
import { Button } from "../../ui/Button";
import { StatusGlyph } from "../../ui/StatusGlyph";
import "../settings/cap.css";
import type { PickAccount } from "./RepoAccounts";
import { RepoEdit } from "./RepoEdit";
import { useReturnFocus } from "./useReturnFocus";
import "./repos.css";

export interface RepoRowProps {
  readonly summary: RepoSummary;
  readonly onRemove: (name: string, deleteClone?: boolean) => Promise<void>;
  readonly accounts?: readonly ForgeAccount[];
  readonly onPickAccount?: PickAccount;
  readonly onEdit: (name: string, input: PatchRepoInput) => Promise<unknown>;
}

function RepoName({ summary }: { readonly summary: RepoSummary }) {
  return (
    <div className="th-repo-name">
      <b>{summary.repo.name}</b>
      <span title={summary.repo.path}>{summary.shortPath}</span>
    </div>
  );
}

function RepoPicks({ summary }: { readonly summary: RepoSummary }) {
  const { name, githubAccount, gitlabAccount } = summary.repo;
  const picks = [
    { forge: "github", login: githubAccount },
    { forge: "gitlab", login: gitlabAccount },
  ];
  return (
    <>
      {picks.map(({ forge, login }) =>
        login === null ? null : (
          <span key={forge} title={`${forge} account for ${name}`}>{`${forge} · ${login}`}</span>
        ),
      )}
    </>
  );
}

function RepoAgents({ summary }: { readonly summary: RepoSummary }) {
  return (
    <span className="th-cap th-repo-agents">
      {summary.glyphs.length > 0 ? (
        <span className="th-glyphs" aria-hidden="true">
          {summary.glyphs.map((glyph, index) => (
            <StatusGlyph key={index} state={glyph} />
          ))}
        </span>
      ) : null}
      {summary.agents}
      {summary.spokenAgents ? <span className="th-sr-only">{`: ${summary.spokenAgents}`}</span> : null}
    </span>
  );
}

export function RepoRow({ summary, onRemove, accounts = [], onPickAccount, onEdit }: RepoRowProps) {
  const [editing, setEditing] = useState(false);
  const focus = useReturnFocus(editing);
  const name = summary.repo.name;
  const close = () => {
    focus.arm();
    setEditing(false);
  };
  return (
    <div className="th-repo">
      <RepoName summary={summary} />
      <div className="th-repo-meta">
        <span title={summary.repo.baseBranch}>{summary.repo.baseBranch}</span>
        <span>{summary.conversations}</span>
        <RepoPicks summary={summary} />
      </div>
      <RepoAgents summary={summary} />
      <div className="th-repo-actions">
        <Link className="th-btn th-btn--ghost th-btn--sm" to={PATHS.home}>
          new conversation
        </Link>
        <Button ref={focus.target} variant="ghost" size="sm" aria-label={`edit ${name}`} onClick={() => setEditing(true)}>
          edit
        </Button>
      </div>
      {editing ? (
        <RepoEdit
          repo={summary.repo}
          managed={summary.managed}
          accounts={accounts}
          onSave={onEdit}
          onPickAccount={onPickAccount}
          onRemove={onRemove}
          onCancel={close}
        />
      ) : null}
    </div>
  );
}
