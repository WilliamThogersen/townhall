export const PLUGIN_API_PREFIX = "/api/plugin/v1/";

const METHODS = ["GET", "POST", "PUT", "DELETE"] as const;

type Method = (typeof METHODS)[number];

interface Route {
  readonly method: Method;
  readonly pattern: RegExp;
  readonly scope: string | null;
}

const route = (method: Method, pattern: string, scope: string | null): Route => ({
  method,
  pattern: new RegExp(`^${pattern}$`),
  scope,
});

const ROUTES: readonly Route[] = [
  route("GET", "info", null),
  route("GET", "tasks", "tasks:read"),
  route("GET", "tasks/[^/]+", "tasks:read"),
  route("GET", "tasks/[^/]+/artifacts", "tasks:read"),
  route("GET", "agents", "tasks:read"),
  route("GET", "stats", "tasks:read"),
  route("POST", "conversations/[^/]+/tasks", "tasks:create"),
  route("GET", "conversations", "conversations:read"),
  route("GET", "conversations/[^/]+", "conversations:read"),
  route("GET", "search", "conversations:read"),
  route("POST", "conversations/[^/]+/messages", "conversations:write"),
  route("GET", "settings", "settings:read"),
  route("GET", "profile", "profile:read"),
  route("GET", "library/agents", "library:read"),
  route("GET", "library/agents/[^/]+", "library:read"),
  route("PUT", "library/agents/[^/]+", "library:write"),
  route("GET", "storage/[^/]+", "storage"),
  route("PUT", "storage/[^/]+", "storage"),
  route("DELETE", "storage/[^/]+", "storage"),
];

export type ResolvedCall =
  | { readonly ok: true; readonly scope: string | null; readonly path: string }
  | { readonly ok: false; readonly reason: string };

const outside = (path: string): ResolvedCall => ({
  ok: false,
  reason: `path ${JSON.stringify(path)} is not under ${PLUGIN_API_PREFIX}`,
});

const FORBIDDEN_CHARS = /[\\#\u0000-\u001f\u007f]/;

function hasOddSegment(pathname: string): boolean {
  return pathname
    .split("/")
    .slice(1)
    .some((part) => {
      const decoded = decodeURIComponent(part);
      return part === "" || decoded === "" || decoded === "." || decoded === ".." || decoded.includes("/");
    });
}

function isMethod(method: string): method is Method {
  return (METHODS as readonly string[]).includes(method);
}

export function resolveCall(method: string, path: string): ResolvedCall {
  if (!path.startsWith(PLUGIN_API_PREFIX) || FORBIDDEN_CHARS.test(path)) return outside(path);
  const verb = method.toUpperCase();
  if (!isMethod(verb)) return { ok: false, reason: `method ${JSON.stringify(method)} is not allowed` };
  const [pathname = "", ...query] = path.split("?");
  if (!isDecodable(pathname) || hasOddSegment(pathname)) return outside(path);
  const rest = pathname.slice(PLUGIN_API_PREFIX.length);
  const named = ROUTES.filter((r) => r.pattern.test(rest));
  if (named.length === 0) return { ok: false, reason: `no such plugin api route: ${path}` };
  const found = named.find((r) => r.method === verb);
  if (!found) return { ok: false, reason: `${verb} is not allowed on ${pathname}` };
  const search = query.length === 0 ? "" : `?${query.join("?")}`;
  return { ok: true, scope: found.scope, path: `${pathname.slice("/api".length)}${search}` };
}

function isDecodable(pathname: string): boolean {
  return !pathname.split("/").some((part) => /%(?![0-9a-fA-F]{2})/.test(part));
}

const TOPIC_SCOPES: Readonly<Record<string, string>> = {
  taskUpdated: "tasks:read",
  agentUpdated: "tasks:read",
  entryAppended: "conversations:read",
  conversationUpdated: "conversations:read",
};

export function eventScopes(topic: string): readonly string[] | null {
  const scope = TOPIC_SCOPES[topic];
  return scope === undefined ? null : ["events", scope];
}
