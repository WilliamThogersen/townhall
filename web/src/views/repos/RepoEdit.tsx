import { useState, type FormEvent, type KeyboardEvent } from "react";
import { toMessage } from "../../api/errors";
import type { PatchRepoInput, Repo } from "../../api/types";
import { canSave, draftOf, patchOf, type EditRepoDraft } from "../../repos/editRepoDraft";
import { Button } from "../../ui/Button";
import { Field, useFocusOn } from "./RepoField";
import "./repos.css";

export interface RepoEditProps {
  readonly repo: Repo;
  readonly onSave: (name: string, input: PatchRepoInput) => Promise<unknown>;
  readonly onCancel: () => void;
}

export function RepoEdit({ repo, onSave, onCancel }: RepoEditProps) {
  const [draft, setDraft] = useState<EditRepoDraft>(() => draftOf(repo));
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nameRef = useFocusOn(0);
  const set = (key: "name" | "baseBranch" | "path") => (value: string) => setDraft((current) => ({ ...current, [key]: value }));
  const patch = patchOf(repo, draft);
  const pathChanged = draft.path.trim() !== repo.path;
  const submit = async (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (patch === null || !canSave(repo, draft) || busy) return;
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
  const onKeyDown = (e: KeyboardEvent<HTMLFormElement>) => {
    if (e.key !== "Escape" || busy) return;
    e.preventDefault();
    onCancel();
  };
  return (
    <form className="th-repo-add" data-layout="row" aria-label={`edit ${repo.name}`} onSubmit={(e) => void submit(e)} onKeyDown={onKeyDown}>
      <Field label="folder" className="th-repo-field--path" value={draft.path} placeholder={repo.path} onChange={set("path")} />
      <Field label="name" value={draft.name} placeholder={repo.name} inputRef={nameRef} onChange={set("name")} />
      <Field label="base branch" value={draft.baseBranch} placeholder={repo.baseBranch} onChange={set("baseBranch")} />
      <div className="th-repo-add-actions">
        <Button type="submit" busy={busy} busyLabel="saving…" disabled={!canSave(repo, draft)}>
          save
        </Button>
        <Button variant="ghost" disabled={busy} onClick={onCancel}>
          cancel
        </Button>
      </div>
      {pathChanged ? (
        <label className="th-cap th-repo-add-error th-repo-add-check">
          <input type="checkbox" checked={draft.moveFolder} disabled={busy} onChange={(e) => setDraft((current) => ({ ...current, moveFolder: e.target.checked }))} />{" "}
          move the folder on disk to this path (it must not exist yet); otherwise the repo is only pointed at it
        </label>
      ) : null}
      {error ? (
        <p className="th-field-error th-repo-add-error" role="alert">
          error: {error}
        </p>
      ) : null}
    </form>
  );
}
