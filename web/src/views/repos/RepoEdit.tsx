import { useState, type FormEvent, type KeyboardEvent } from "react";
import { createPortal } from "react-dom";
import { toMessage } from "../../api/errors";
import type { PatchRepoInput, Repo } from "../../api/types";
import { canSave, draftOf, patchOf, type EditRepoDraft } from "../../repos/editRepoDraft";
import type { ForgeAccount } from "../../settings/settings";
import { Button } from "../../ui/Button";
import { RemoveConfirm } from "./RemoveConfirm";
import { RepoAccounts, type PickAccount } from "./RepoAccounts";
import { Field, useFocusOn } from "./RepoField";
import { useReturnFocus } from "./useReturnFocus";
import "./repos.css";

export interface RepoEditProps {
  readonly repo: Repo;
  readonly managed: boolean;
  readonly accounts: readonly ForgeAccount[];
  readonly onSave: (name: string, input: PatchRepoInput) => Promise<unknown>;
  readonly onPickAccount?: PickAccount;
  readonly onRemove: (name: string, deleteClone?: boolean) => Promise<void>;
  readonly onCancel: () => void;
}

export function RepoEdit({ repo, managed, accounts, onSave, onPickAccount, onRemove, onCancel }: RepoEditProps) {
  const [draft, setDraft] = useState<EditRepoDraft>(() => draftOf(repo));
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [removing, setRemoving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nameRef = useFocusOn(0);
  const focus = useReturnFocus(confirming);
  const set = (key: "name" | "baseBranch" | "path") => (value: string) => setDraft((current) => ({ ...current, [key]: value }));
  const patch = patchOf(repo, draft);
  const pathChanged = draft.path.trim() !== repo.path;
  const locked = busy || removing;
  const submit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (patch === null || !canSave(repo, draft) || locked) return;
    setBusy(true);
    setError(null);
    try {
      await onSave(repo.name, patch);
    } catch (failure) {
      setError(toMessage(failure));
      setBusy(false);
      return;
    }
    onCancel();
  };
  const cancelRemove = () => {
    focus.arm();
    setConfirming(false);
  };
  const remove = async (deleteClone: boolean) => {
    setRemoving(true);
    await onRemove(repo.name, deleteClone);
    onCancel();
  };
  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "Escape" || e.defaultPrevented || locked) return;
    e.preventDefault();
    onCancel();
  };
  return createPortal(
    <div className="th-repo-modal">
      <div className="th-repo-modal-box" role="dialog" aria-modal="true" aria-label={`edit ${repo.name}`} onKeyDown={onKeyDown}>
        <form className="th-repo-add" data-layout="stack" onSubmit={(e) => void submit(e)}>
          <Field label="folder" className="th-repo-field--path" value={draft.path} placeholder={repo.path} onChange={set("path")} />
          <Field label="name" value={draft.name} placeholder={repo.name} inputRef={nameRef} onChange={set("name")} />
          <Field label="base branch" value={draft.baseBranch} placeholder={repo.baseBranch} onChange={set("baseBranch")} />
          {pathChanged ? (
            <label className="th-cap th-repo-add-error th-repo-add-check">
              <input type="checkbox" checked={draft.moveFolder} disabled={locked} onChange={(e) => setDraft((current) => ({ ...current, moveFolder: e.target.checked }))} />{" "}
              move the folder on disk to this path (it must not exist yet); otherwise the repo is only pointed at it
            </label>
          ) : null}
          {onPickAccount !== undefined && accounts.length > 0 ? <RepoAccounts repo={repo} accounts={accounts} onPick={onPickAccount} /> : null}
          {error ? (
            <p className="th-field-error th-repo-add-error" role="alert">
              error: {error}
            </p>
          ) : null}
          {confirming ? (
            <RemoveConfirm name={repo.name} managed={managed} busy={removing} onCancel={cancelRemove} onConfirm={(deleteClone) => void remove(deleteClone)} />
          ) : (
            <div className="th-repo-add-actions">
              <Button ref={focus.target} variant="ghost" className="th-repo-remove" disabled={locked} aria-label={`remove ${repo.name}`} onClick={() => setConfirming(true)}>
                remove
              </Button>
              <Button type="submit" busy={busy} busyLabel="saving…" disabled={!canSave(repo, draft) || removing}>
                save
              </Button>
              <Button variant="ghost" disabled={locked} onClick={onCancel}>
                cancel
              </Button>
            </div>
          )}
        </form>
      </div>
    </div>,
    document.body,
  );
}
