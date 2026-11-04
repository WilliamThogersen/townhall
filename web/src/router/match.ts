export type RouteName =
  | "home"
  | "conversation"
  | "control"
  | "repos"
  | "settings"
  | "search"
  | "not-found";

export interface Route {
  readonly name: RouteName;
  readonly path: string;
  readonly params: Readonly<Record<string, string>>;
  readonly query: Readonly<Record<string, string>>;
}

export const PATHS = {
  home: "/",
  control: "/townhall",
  repos: "/repos",
  settings: "/settings",
  search: "/search",
  conversation: (id: string) => `/c/${encodeURIComponent(id)}`,
} as const;

const TABLE: readonly { readonly name: RouteName; readonly pattern: string }[] = [
  { name: "home", pattern: PATHS.home },
  { name: "conversation", pattern: "/c/:id" },
  { name: "control", pattern: PATHS.control },
  { name: "repos", pattern: PATHS.repos },
  { name: "settings", pattern: PATHS.settings },
  { name: "search", pattern: PATHS.search },
];

function segments(path: string): string[] {
  return path.split("/").filter((part, i, all) => i > 0 && !(part === "" && i === all.length - 1));
}

function decode(part: string): string {
  try {
    return decodeURIComponent(part);
  } catch {
    return part;
  }
}

function matchPattern(pattern: string, path: string): Record<string, string> | null {
  const want = segments(pattern);
  const got = segments(path);
  if (want.length !== got.length) return null;
  const entries: [string, string][] = [];
  for (const [i, part] of want.entries()) {
    const actual = got[i] ?? "";
    if (part.startsWith(":")) {
      if (actual === "") return null;
      entries.push([part.slice(1), decode(actual)]);
    } else if (part !== actual) {
      return null;
    }
  }
  return Object.fromEntries(entries);
}

export function matchRoute(path: string, search = ""): Route {
  const query = Object.fromEntries(new URLSearchParams(search));
  for (const { name, pattern } of TABLE) {
    const params = matchPattern(pattern, path);
    if (params) return { name, path, params, query };
  }
  return { name: "not-found", path, params: {}, query };
}
