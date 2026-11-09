import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Button } from "../../ui/Button";
import "./repos.css";

export function removeQuestion(name: string, managed = false): string {
  const stay = managed ? "the clone stays on disk unless you tick the box" : "the folder and its worktrees stay on disk";
  return `remove ${name}? ${stay}`;
}

export interface RemoveConfirmProps {
  readonly name: string;
  readonly managed: boolean;
  readonly busy: boolean;
  readonly onCancel: () => void;
  readonly onConfirm: (deleteClone: boolean) => void;
}

export function RemoveConfirm({ name, managed, busy, onCancel, onConfirm }: RemoveConfirmProps) {
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
