import { useState, type FormEvent, type KeyboardEvent } from "react";
import type { AddRepoInput } from "../../api/types";
import {
  addProgress,
  canSubmit,
  cloneTargetOf,
  DEFAULT_CLONES_DIR,
  draftToInput,
  EMPTY_DRAFT,
  isLink,
  linkProblem,
  type AddRepoDraft,
} from "../../repos/addRepoDraft";
import { Button } from "../../ui/Button";
import { Field, useFocusOn } from "./RepoField";
import "./repos.css";

export type AddRepoLayout = "row" | "stack";

export interface AddRepoFormProps {
  readonly busy: boolean;
  readonly error: string | null;
  readonly focusToken: number;
  readonly layout?: AddRepoLayout;
  /** The `clonesDir` setting: where a link is cloned when the form names no folder. */
  readonly defaultCloneDir?: string;
  readonly onSubmit: (input: AddRepoInput) => void;
  readonly onCancel: () => void;
}

export function AddRepoForm({ busy, error, focusToken, layout = "row", defaultCloneDir = "", onSubmit, onCancel }: AddRepoFormProps) {
  const [draft, setDraft] = useState<AddRepoDraft>(EMPTY_DRAFT);
  const pathRef = useFocusOn(focusToken);
  const set = (key: keyof AddRepoDraft) => (value: string) => setDraft((current) => ({ ...current, [key]: value }));
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (canSubmit(draft) && !busy) onSubmit(draftToInput(draft));
  };
  const onKeyDown = (e: KeyboardEvent<HTMLFormElement>) => {
    if (e.key !== "Escape" || busy) return;
    e.preventDefault();
    onCancel();
  };
  const link = isLink(draft.path);
  const target = cloneTargetOf(draft, defaultCloneDir);
  const problem = error === null ? linkProblem(draft.path) : null;
  return (
    <form className="th-repo-add" data-layout={layout} aria-label="add repository" onSubmit={submit} onKeyDown={onKeyDown}>
      <Field label="folder or link" className="th-repo-field--path" value={draft.path} placeholder="~/Developer/shop or a github/gitlab link" inputRef={pathRef} onChange={set("path")} />
      <Field label="name" value={draft.name} placeholder="folder name" onChange={set("name")} />
      <Field label="base branch" value={draft.baseBranch} placeholder="auto" onChange={set("baseBranch")} />
      <div className="th-repo-add-actions">
        <Button type="submit" busy={busy} busyLabel="adding…" disabled={!canSubmit(draft)}>
          add
        </Button>
        <Button variant="ghost" disabled={busy} onClick={onCancel}>
          cancel
        </Button>
      </div>
      {link ? (
        <Field label="clone into" className="th-repo-field--wide" value={draft.cloneDir} placeholder={defaultCloneDir.trim() || DEFAULT_CLONES_DIR} onChange={set("cloneDir")} />
      ) : null}
      {link && target !== null && !busy ? <p className="th-cap th-repo-add-error">clones into {target}</p> : null}
      {busy ? (
        <p className="th-cap th-repo-add-error" role="status">
          {addProgress(draft)}
          {target === null ? "" : ` ${target}`}
        </p>
      ) : null}
      {problem !== null ? (
        <p className="th-field-error th-repo-add-error" role="alert">
          error: {problem}
        </p>
      ) : null}
      {error ? (
        <p className="th-field-error th-repo-add-error" role="alert">
          error: {error}
        </p>
      ) : null}
    </form>
  );
}
