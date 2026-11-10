export type PermissionGroupId = "runs" | "agents" | "api" | "ui" | "env" | "other";

export interface PermissionItem {
  readonly raw: string;
  readonly text: string;
  readonly flag: string | null;
}

export interface PermissionGroup {
  readonly id: PermissionGroupId;
  readonly title: string;
  readonly items: readonly PermissionItem[];
}

const TITLES: Readonly<Record<PermissionGroupId, string>> = {
  runs: "runs programs on this machine, as you",
  agents: "changes what agents do",
  api: "reads or writes townhall data",
  ui: "shows UI in these places",
  env: "reads these environment variables",
  other: "other",
};

const ORDER: readonly PermissionGroupId[] = ["runs", "agents", "api", "ui", "env", "other"];

const GROUP_OF: Readonly<Record<string, PermissionGroupId>> = {
  hook: "runs",
  "runs.hook": "runs",
  "runs.mcp": "runs",
  "connects.mcp": "runs",
  "claude.hooks": "agents",
  "claude.hooks.agent-type": "agents",
  "claude.mcp": "agents",
  "claude.mcp.read-only": "agents",
  "claude.planner": "agents",
  prompt: "agents",
  api: "api",
  "claude.api": "api",
  ui: "ui",
  env: "env",
};

const AGENT_API_FLAG = "an agent that is prompt injected could use this";
const PRE_TOOL_USE_FLAG = "can block or change any tool an agent calls";

function split(raw: string): { readonly kind: string; readonly rest: string } {
  const at = raw.indexOf(":");
  return at < 0 ? { kind: raw, rest: "" } : { kind: raw.slice(0, at), rest: raw.slice(at + 1) };
}

function pair(rest: string): readonly [string, string] {
  const at = rest.indexOf(":");
  return at < 0 ? [rest, ""] : [rest.slice(0, at), rest.slice(at + 1)];
}

function mcpTools(tool: string, readOnly: boolean): string {
  const which = tool === "*" ? "all tools" : `tool ${tool}`;
  return readOnly ? `${which} (read only)` : which;
}

export function describePermission(raw: string): PermissionItem {
  const { kind, rest } = split(raw);
  const [first, second] = pair(rest);
  const item = (text: string, flag: string | null = null): PermissionItem => ({ raw, text, flag });
  switch (kind) {
    case "hook":
      return item(`hook on ${rest}`);
    case "runs.hook":
      return item(`hook ${first} runs ${second}`);
    case "runs.mcp":
      return item(`mcp server ${first} runs ${second}`);
    case "connects.mcp":
      return item(`mcp server ${first} connects to ${second}`);
    case "claude.hooks":
      return item(`hook ${rest} on agents`, rest === "PreToolUse" ? PRE_TOOL_USE_FLAG : null);
    case "claude.hooks.agent-type":
      return item(`hooks for agent type ${rest}`);
    case "claude.mcp":
      return item(`mcp server ${first}, ${mcpTools(second, false)}`);
    case "claude.mcp.read-only":
      return item(`mcp server ${first}, ${mcpTools(second, true)}`);
    case "claude.planner":
      return item("changes townhall's planner");
    case "claude.api":
      return item(`${rest}, for agents`, AGENT_API_FLAG);
    case "prompt":
      return item(rest === "brief" ? "adds to the brief" : `adds a ${rest} prompt`);
    default:
      return item(rest === "" ? raw : rest);
  }
}

export function groupPermissions(permissions: readonly string[]): readonly PermissionGroup[] {
  const items = permissions.map((raw) => ({ id: GROUP_OF[split(raw).kind] ?? "other", item: describePermission(raw) }));
  return ORDER.flatMap((id) => {
    const own = items.filter((entry) => entry.id === id).map((entry) => entry.item);
    return own.length === 0 ? [] : [{ id, title: TITLES[id], items: own }];
  });
}
