import type { RepoSummary } from "./repoSummary";

export function filterRepos(summaries: readonly RepoSummary[], query: string): readonly RepoSummary[] {
  const needle = query.trim().toLowerCase();
  if (needle === "") return summaries;
  return summaries.filter(({ repo }) => repo.name.toLowerCase().includes(needle) || repo.path.toLowerCase().includes(needle));
}
