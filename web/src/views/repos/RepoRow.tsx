import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { PatchRepoInput } from "../../api/types";
import type { RepoSummary } from "../../repos/repoSummary";
import type { ForgeAccount } from "../../settings/settings";
import { Link, PATHS } from "../../router";
import { Button } from "../../ui/Button";
import { StatusGlyph } from "../../ui/StatusGlyph";
import "../settings/cap.css";
import { RepoAccounts, type PickAccount } from "./RepoAccounts";
import { RepoEdit } from "./RepoEdit";
import "./repos.css";

export function removeQuestion(name: string, managed = false): string {
  const stay = managed ? "the clone stays on disk unless you tick the box" : "the folder and its worktrees stay on disk";
  return `remove ${name}? ${stay}`;
}

export interface RepoRowProps {
  readonly summary: RepoSummary;
  readonly onRemove: (name: string, deleteClone?: boolean) => Promise<void>;
  readonly accounts?: readonly ForgeAccount[];
  readonly onPickAccount?: PickAccount;
  /** Saves an edit; rejects with the daemon's reason, which the edit form shows. */
  readonly onEdit?: (name: string, input: PatchRepoInput) => Promise<unknown>;
}

function RepoName({ summary }: { readonly summary: RepoSummary }) {
  return (
    <div className="th-repo-name">
      <b>{summary.repo.name}</b>
      <span title={summary.repo.path}>{summary.shortPath}</span>
    </div>
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

interface ConfirmProps {
  readonly name: string;
  readonly managed: boolean;
  readonly busy: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: (deleteClone: boolean) => void;
}

function RemoveConfirm({ name, managed, busy, onCancel, onConfirm }: ConfirmProps) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const [deleteClone, setDeleteClone] = useState(false);
  useEffect(() => cancelRef.current?.focus(), []);
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Escape" || busy) return;
    e.preventDefault();
    onCancel();
  };
  return (
    <div className="th-repo-confirm" role="group" aria-label={`remove ${name}`} onKeyDown={onKeyDown}>
      <span className="th-confirm">
        <span>{removeQuestion(name, managed)}</span>
      </span>
      {managed ? (
        <label>
          <input type="checkbox" checked={deleteClone} disabled={busy} onChange={(e) => setDeleteClone(e.target.checked)} />{" "}
          also delete the clone
        </label>
      ) : null}
      <Button ref={cancelRef} variant="ghost" size="sm" disabled={busy} onClick={onCancel}>
        cancel
      </Button>
      <Button variant="danger-solid" size="sm" busy={busy} busyLabel="removing…" onClick={() => onConfirm(deleteClone)}>
        remove
      </Button>
    </div>
  );
}

function useReturnFocus(confirming: boolean) {
  const target = useRef<HTMLButtonElement>(null);
  const pending = useRef(false);
  useEffect(() => {
    if (confirming || !pending.current) return;
    pending.current = false;
    target.current?.focus();
  }, [confirming]);
  const arm = () => {
    pending.current = true;
  };
  return { target, arm };
}

export function RepoRow({ summary, onRemove, accounts = [], onPickAccount, onEdit }: RepoRowProps) {
  const [confirming, setConfirming] = useState(false);
  const [editing, setEditing] = useState(false);
  const [busy, setBusy] = useState(false);
  const focus = useReturnFocus(confirming);
  const name = summary.repo.name;
  const cancel = () => {
    focus.arm();
    setConfirming(false);
  };
  const confirm = async (deleteClone: boolean) => {
    setBusy(true);
    await onRemove(name, deleteClone);
    focus.arm();
    setBusy(false);
    setConfirming(false);
  };
  if (confirming) {
    return (
      <div className="th-repo" data-confirm="true">
        <RepoName summary={summary} />
        <RemoveConfirm name={name} managed={summary.managed} busy={busy} onCancel={cancel} onConfirm={(deleteClone) => void confirm(deleteClone)} />
      </div>
    );
  }
  if (editing && onEdit !== undefined) {
    return <RepoEdit repo={summary.repo} onSave={onEdit} onCancel={() => setEditing(false)} />;
  }
  const choose = onPickAccount !== undefined && accounts.length > 0;
  return (
    <div className="th-repo" data-accounts={choose ? "true" : undefined}>
      <RepoName summary={summary} />
      <div className="th-repo-meta">
        <span title={summary.repo.baseBranch}>{summary.repo.baseBranch}</span>
        <span>{summary.conversations}</span>
      </div>
      <RepoAgents summary={summary} />
      <div className="th-repo-actions">
        <Link className="th-btn th-btn--ghost th-btn--sm" to={PATHS.home}>
          new conversation
        </Link>
        {onEdit === undefined ? null : (
          <Button variant="ghost" size="sm" aria-label={`edit ${name}`} onClick={() => setEditing(true)}>
            edit
          </Button>
        )}
        <Button ref={focus.target} variant="ghost" size="sm" aria-label={`remove ${name}`} onClick={() => setConfirming(true)}>
          remove
        </Button>
      </div>
      {choose ? <RepoAccounts repo={summary.repo} accounts={accounts} onPick={onPickAccount} /> : null}
    </div>
  );
}
