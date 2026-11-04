import type { Repo } from "../../api/types";

export const PREVIEW_STARTERS: readonly string[] = [
  "townhall needs a settings page",
  "fix the failing tests on shop main",
  "find slow queries in search",
];

const TEMPLATES: readonly ((repo: Repo) => string)[] = [
  (repo) => `fix the failing tests on ${repo.name} ${repo.baseBranch}`,
  (repo) => `find dead code in ${repo.name}`,
  (repo) => `tidy the readme in ${repo.name}`,
];

export function starterPrompts(repos: readonly Repo[] | null): readonly string[] {
  if (!repos || repos.length === 0) return PREVIEW_STARTERS;
  return TEMPLATES.map((template, i) => template(repos[i % repos.length]!));
}
