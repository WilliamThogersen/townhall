import { createPortal } from "react-dom";
import { AddRepoForm, type AddRepoFormProps } from "./AddRepoForm";
import "./repos.css";

export type AddRepoModalProps = Omit<AddRepoFormProps, "layout">;

export function AddRepoModal(props: AddRepoModalProps) {
  return createPortal(
    <div className="th-repo-modal">
      <div className="th-repo-modal-box" role="dialog" aria-modal="true" aria-label="add repository">
        <AddRepoForm {...props} layout="stack" />
      </div>
    </div>,
    document.body,
  );
}
