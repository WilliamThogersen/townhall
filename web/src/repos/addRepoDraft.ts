import type { AddRepoInput } from "../api/types";

export interface AddRepoDraft {
  readonly path: string;
  readonly name: string;
  readonly baseBranch: string;
}

export const EMPTY_DRAFT: AddRepoDraft = { path: "", name: "", baseBranch: "" };

export function canSubmit(draft: AddRepoDraft): boolean {
  return draft.path.trim() !== "";
}

export function draftToInput(draft: AddRepoDraft): AddRepoInput {
  const name = draft.name.trim();
  const baseBranch = draft.baseBranch.trim();
  return {
    path: draft.path.trim(),
    ...(name === "" ? {} : { name }),
    ...(baseBranch === "" ? {} : { baseBranch }),
  };
}
