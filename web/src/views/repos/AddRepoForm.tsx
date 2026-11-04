import { useEffect, useRef, useState, type ChangeEvent, type FormEvent, type KeyboardEvent, type Ref } from "react";
import type { AddRepoInput } from "../../api/types";
import { canSubmit, draftToInput, EMPTY_DRAFT, type AddRepoDraft } from "../../repos/addRepoDraft";
import { Button } from "../../ui/Button";
import "./repos.css";

export type AddRepoLayout = "row" | "stack";

export interface AddRepoFormProps {
  readonly busy: boolean;
  readonly error: string | null;
  readonly focusToken: number;
  readonly layout?: AddRepoLayout;
  readonly onSubmit: (input: AddRepoInput) => void;
  readonly onCancel: () => void;
}

interface FieldProps {
  readonly label: string;
  readonly value: string;
  readonly placeholder: string;
  readonly className?: string;
  readonly inputRef?: Ref<HTMLInputElement>;
  readonly onChange: (value: string) => void;
}

function Field({ label, value, placeholder, className, inputRef, onChange }: FieldProps) {
  return (
    <label className={className ? `th-repo-field ${className}` : "th-repo-field"}>
      <span>{label}</span>
      <input
        ref={inputRef}
        className="th-input"
        value={value}
        placeholder={placeholder}
        autoComplete="off"
        autoCapitalize="off"
        spellCheck={false}
        onChange={(e: ChangeEvent<HTMLInputElement>) => onChange(e.target.value)}
      />
    </label>
  );
}

function useFocusOn(token: number) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    ref.current?.focus();
  }, [token]);
  return ref;
}

export function AddRepoForm({ busy, error, focusToken, layout = "row", onSubmit, onCancel }: AddRepoFormProps) {
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
  return (
    <form className="th-repo-add" data-layout={layout} aria-label="add repository" onSubmit={submit} onKeyDown={onKeyDown}>
      <Field label="path" className="th-repo-field--path" value={draft.path} placeholder="~/Developer/shop" inputRef={pathRef} onChange={set("path")} />
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
      {error ? (
        <p className="th-field-error th-repo-add-error" role="alert">
          error: {error}
        </p>
      ) : null}
    </form>
  );
}
