import type { PatchRepoInput, Repo } from "../api/types";

export interface EditRepoDraft {
  readonly name: string;
  readonly baseBranch: string;
  readonly path: string;
  readonly moveFolder: boolean;
}

const NAME = /^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$/;

export function draftOf(repo: Repo): EditRepoDraft {
  return { name: repo.name, baseBranch: repo.baseBranch, path: repo.path, moveFolder: false };
}

export function patchOf(repo: Repo, draft: EditRepoDraft): PatchRepoInput | null {
  const name = draft.name.trim();
  const baseBranch = draft.baseBranch.trim();
  const path = draft.path.trim();
  const patch: PatchRepoInput = {
    ...(name === repo.name ? {} : { name }),
    ...(baseBranch === repo.baseBranch ? {} : { baseBranch }),
    ...(path === repo.path ? {} : { path, ...(draft.moveFolder ? { moveFolder: true } : {}) }),
  };
  return Object.keys(patch).length === 0 ? null : patch;
}

export function canSave(repo: Repo, draft: EditRepoDraft): boolean {
  const filled = draft.name.trim() !== "" && draft.baseBranch.trim() !== "" && draft.path.trim() !== "";
  return filled && NAME.test(draft.name.trim()) && patchOf(repo, draft) !== null;
}
