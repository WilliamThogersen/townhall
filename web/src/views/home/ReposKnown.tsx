import type { Repo } from "../../api/types";
import { Link, PATHS } from "../../router";

export function reposKnownLabel(count: number): string {
  return `${count} ${count === 1 ? "repo" : "repos"} known`;
}

export function ReposKnown({ repos }: { readonly repos: readonly Repo[] }) {
  return (
    <Link className="th-repos-known" to={PATHS.repos} title={repos.map((repo) => repo.name).join(", ")}>
      {reposKnownLabel(repos.length)}
    </Link>
  );
}
