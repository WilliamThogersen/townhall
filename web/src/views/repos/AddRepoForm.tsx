import { useState, type FormEvent, type KeyboardEvent } from "react";
import type { AddRepoInput } from "../../api/types";
import {
  addProgress,
  cloneTargetOf,
  DEFAULT_CLONES_DIR,
  DEFAULT_PROTOCOL,
  draftToInput,
  EMPTY_DRAFT,
  modeProblem,
  withProtocol,
  type AddMode,
  type AddRepoDraft,
  type CloneProtocol,
} from "../../repos/addRepoDraft";
import { Button } from "../../ui/Button";
import { Segmented, type SegmentedOption } from "../../ui/Segmented";
import { Field, useFocusOn } from "./RepoField";
import "./repos.css";

export type AddRepoLayout = "row" | "stack";

const MODES: readonly SegmentedOption<AddMode>[] = [
  { value: "folder", label: "folder" },
  { value: "url", label: "url" },
];

const PROTOCOLS: readonly SegmentedOption<CloneProtocol>[] = [
  { value: "ssh", label: "ssh" },
  { value: "https", label: "https" },
];

export interface AddRepoFormProps {
  readonly busy: boolean;
  readonly error: string | null;
  readonly focusToken: number;
  readonly layout?: AddRepoLayout;
  readonly defaultCloneDir?: string;
  readonly onBrowse?: () => Promise<string | null>;
  readonly onSubmit: (input: AddRepoInput) => void;
  readonly onCancel: () => void;
}

export function AddRepoForm({ busy, error, focusToken, layout = "row", defaultCloneDir = "", onBrowse, onSubmit, onCancel }: AddRepoFormProps) {
  const [draft, setDraft] = useState<AddRepoDraft>(EMPTY_DRAFT);
  const [mode, setMode] = useState<AddMode>("folder");
  const [protocol, setProtocol] = useState<CloneProtocol>(DEFAULT_PROTOCOL);
  const pathRef = useFocusOn(focusToken);
  const set = (key: keyof AddRepoDraft) => (value: string) => setDraft((current) => ({ ...current, [key]: value }));
  const problem = modeProblem(mode, draft.path);
  const ready = draft.path.trim() !== "" && problem === null;
  const submit = (e: FormEvent<HTMLFormElement>) => {
    e.preventDefault();
    if (ready && !busy) onSubmit(draftToInput(draft, protocol));
  };
  const onKeyDown = (e: KeyboardEvent<HTMLFormElement>) => {
    if (e.key !== "Escape" || busy) return;
    e.preventDefault();
    onCancel();
  };
  const choose = (next: AddMode) => {
    setMode(next);
    setDraft((current) => ({ ...current, path: "" }));
  };
  const browse = async () => {
    const path = await onBrowse?.();
    if (path !== null && path !== undefined) set("path")(path);
  };
  const url = mode === "url";
  const target = url ? cloneTargetOf(draft, defaultCloneDir) : null;
  return (
    <form className="th-repo-add" data-layout={layout} aria-label="add repository" onSubmit={submit} onKeyDown={onKeyDown}>
      <Segmented name="th-repo-add-mode" label="add from" value={mode} options={MODES} onChange={choose} />
      {url ? <Segmented name="th-repo-add-protocol" label="clone over" value={protocol} options={PROTOCOLS} onChange={setProtocol} /> : null}
      {url ? (
        <Field label="git url" className="th-repo-field--wide" value={draft.path} placeholder="a github or gitlab link" inputRef={pathRef} onChange={set("path")} />
      ) : (
        <Field label="folder" className="th-repo-field--wide" value={draft.path} placeholder="~/Developer/shop" inputRef={pathRef} onChange={set("path")} />
      )}
      {!url && onBrowse ? (
        <Button disabled={busy} onClick={() => void browse()}>
          browse…
        </Button>
      ) : null}
      <Field label="name" value={draft.name} placeholder="folder name" onChange={set("name")} />
      <Field label="base branch" value={draft.baseBranch} placeholder="auto" onChange={set("baseBranch")} />
      <div className="th-repo-add-actions">
        <Button type="submit" busy={busy} busyLabel="adding…" disabled={!ready}>
          add
        </Button>
        <Button variant="ghost" disabled={busy} onClick={onCancel}>
          cancel
        </Button>
      </div>
      {url ? (
        <Field label="clone into" className="th-repo-field--wide" value={draft.cloneDir} placeholder={defaultCloneDir.trim() || DEFAULT_CLONES_DIR} onChange={set("cloneDir")} />
      ) : null}
      {target !== null && !busy ? <p className="th-cap th-repo-add-error">clones {withProtocol(draft.path, protocol)} into {target}</p> : null}
      {busy ? (
        <p className="th-cap th-repo-add-error" role="status">
          {addProgress(draft)}
          {target === null ? "" : ` ${target}`}
        </p>
      ) : null}
      {problem !== null && error === null ? (
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
