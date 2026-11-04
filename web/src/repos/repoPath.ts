const TOWNHALL_DIR = "/.townhall";
const TAIL_FOLDERS = 2;
const HEAD_PARTS = 2;

function trimTrailingSlashes(path: string): string {
  const trimmed = path.replace(/\/+$/, "");
  return trimmed === "" && path.startsWith("/") ? "/" : trimmed;
}

function tildify(path: string, home: string | null): string {
  if (home === null) return path;
  const base = trimTrailingSlashes(home);
  if (base === "/" || base === "") return path;
  if (path === base) return "~";
  return path.startsWith(`${base}/`) ? `~${path.slice(base.length)}` : path;
}

export function shortRepoPath(path: string, home: string | null): string {
  const tilde = tildify(trimTrailingSlashes(path), home);
  const parts = tilde.split("/");
  if (parts.length <= HEAD_PARTS + TAIL_FOLDERS + 1) return tilde;
  return [...parts.slice(0, HEAD_PARTS), "…", ...parts.slice(-TAIL_FOLDERS)].join("/");
}

export function repoName(path: string): string {
  const trimmed = trimTrailingSlashes(path);
  return trimmed.split("/").filter(Boolean).at(-1) ?? trimmed;
}

export function userHomeOf(townhallHome: string): string | null {
  const trimmed = trimTrailingSlashes(townhallHome);
  if (!trimmed.endsWith(TOWNHALL_DIR)) return null;
  const parent = trimmed.slice(0, -TOWNHALL_DIR.length);
  return parent === "" ? null : parent;
}

export function conversationCount(count: number): string {
  if (count === 0) return "no conversations";
  return `${count} conversation${count === 1 ? "" : "s"}`;
}
