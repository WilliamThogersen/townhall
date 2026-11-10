import { AVATAR_PARTS, type AvatarPart } from "../views/agents/avatar";
import type { AvatarSpec } from "../views/agents/avatarSpec";
import { fail, list, listOf, nullable, num, oneOf, record, str, strings, type Check, type Guard } from "./guardKit";
import { request, TomlBody } from "./request";
import type {
  AgentAdapterKind,
  AgentRun,
  AgentRunKind,
  AgentType,
  AgentTypeEffort,
  AgentTypeMode,
  AgentTypeSummary,
  AgentUsage,
  AgentUsageSeries,
  UsageBucket,
  UsageRange,
  UsageShare,
} from "./types";

const adapter = oneOf<AgentAdapterKind>(["claude", "shell"]);
const mode = oneOf<AgentTypeMode>(["default", "acceptEdits", "plan", "bypassPermissions"]);
const effort = oneOf<AgentTypeEffort>(["low", "medium", "high", "xhigh", "max"]);
const origin = oneOf<"user" | "plugin">(["user", "plugin"]);
const range =oneOf<UsageRange>(["24h", "7d", "30d"]);
const runKind = oneOf<AgentRunKind>(["task", "turn"]);

const PART_NAMES = Object.keys(AVATAR_PARTS) as readonly AvatarPart[];

const parseAvatar: Check<AvatarSpec> = (value, what) => {
  const r = record(value, what);
  const parts = PART_NAMES.flatMap((part) => {
    const chosen = nullable(r[part], `${what}.${part}`, oneOf<string>(AVATAR_PARTS[part]));
    return chosen === null ? [] : [[part, chosen] as const];
  });
  const grid = nullable(r.grid, `${what}.grid`, strings);
  return { ...Object.fromEntries(parts), ...(grid === null ? {} : { grid }) } as AvatarSpec;
};

function parseSummary(value: unknown): AgentTypeSummary {
  const r = record(value, "agent type");
  return {
    name: str(r.name, "agentType.name"),
    description: str(r.description, "agentType.description"),
    adapter: adapter(r.adapter, "agentType.adapter"),
    model: nullable(r.model, "agentType.model", str),
    effort: nullable(r.effort, "agentType.effort", effort),
    permissionMode: nullable(r.permissionMode, "agentType.permissionMode", mode),
    avatar: nullable(r.avatar, "agentType.avatar", parseAvatar),
    updatedAt: str(r.updatedAt, "agentType.updatedAt"),
  };
}

function stringMap(value: unknown, what: string): Readonly<Record<string, string>> {
  const r = record(value, what);
  return Object.fromEntries(Object.entries(r).map(([key, item]) => [key, str(item, `${what}.${key}`)]));
}

const parseType: Guard<AgentType> = (value) => {
  const r = record(value, "agent type");
  if (typeof r.source !== "string") return fail("agentType.source");
  return {
    ...parseSummary(r),
    tools: strings(r.tools, "agentType.tools"),
    prompt: nullable(r.prompt, "agentType.prompt", str),
    command: nullable(r.command, "agentType.command", str),
    canHandBackTo: nullable(r.canHandBackTo, "agentType.canHandBackTo", strings),
    env: stringMap(r.env, "agentType.env"),
    source: r.source,
    ...(r.origin === undefined
      ? {}
      : { origin: origin(r.origin, "agentType.origin"), plugin: nullable(r.plugin, "agentType.plugin", str) }),
  };
};

export const listAgentTypes = (signal?: AbortSignal): Promise<readonly AgentTypeSummary[]> =>
  request("GET", "/library/agents", listOf(parseSummary, "agent types"), undefined, signal);

export const getAgentType = (name: string, signal?: AbortSignal): Promise<AgentType> =>
  request("GET", `/library/agents/${encodeURIComponent(name)}`, parseType, undefined, signal);

const parseUsage: Guard<AgentUsage> = (value) => {
  const r = record(value, "agent usage");
  return {
    agent: str(r.agent, "agentUsage.agent"),
    runs: num(r.runs, "agentUsage.runs"),
    tokens: num(r.tokens, "agentUsage.tokens"),
  };
};

const parseRun: Guard<AgentRun> = (value) => {
  const r = record(value, "agent run");
  return {
    kind: runKind(r.kind, "agentRun.kind"),
    conversationId: str(r.conversationId, "agentRun.conversationId"),
    task: str(r.task, "agentRun.task"),
    repo: str(r.repo, "agentRun.repo"),
    state: str(r.state, "agentRun.state"),
    model: nullable(r.model, "agentRun.model", str),
    tokens: num(r.tokens, "agentRun.tokens"),
    createdAt: str(r.createdAt, "agentRun.createdAt"),
    endedAt: nullable(r.endedAt, "agentRun.endedAt", str),
  };
};

export const listAgentRuns = (name: string, signal?: AbortSignal): Promise<readonly AgentRun[]> =>
  request("GET", `/library/agents/${encodeURIComponent(name)}/runs`, listOf(parseRun, "agent runs"), undefined, signal);

export const listAgentUsage =(signal?: AbortSignal): Promise<readonly AgentUsage[]> =>
  request("GET", "/library/agents/usage", listOf(parseUsage, "agent usage"), undefined, signal);

export const saveAgentType = (name: string, source: string, signal?: AbortSignal): Promise<AgentType> =>
  request("PUT", `/library/agents/${encodeURIComponent(name)}`, parseType, new TomlBody(source), signal);

const parseCounts = (value: unknown, what: string): { readonly runs: number; readonly tokens: number } => {
  const r = record(value, what);
  return { runs: num(r.runs, `${what}.runs`), tokens: num(r.tokens, `${what}.tokens`) };
};

const parseBucket: Check<UsageBucket> = (value, what) => ({
  start: str(record(value, what).start, `${what}.start`),
  ...parseCounts(value, what),
});

const parseShare: Check<UsageShare> = (value, what) => ({
  key: str(record(value, what).key, `${what}.key`),
  ...parseCounts(value, what),
});

const parseSeries: Guard<AgentUsageSeries> = (value) => {
  const r = record(value, "agent usage series");
  return {
    agent: str(r.agent, "usage.agent"),
    range: range(r.range, "usage.range"),
    bucketMs: num(r.bucketMs, "usage.bucketMs"),
    since: str(r.since, "usage.since"),
    until: str(r.until, "usage.until"),
    ...parseCounts(r, "usage"),
    buckets: list(r.buckets, "usage.buckets", parseBucket),
    byTask: list(r.byTask, "usage.byTask", parseShare),
    byRepo: list(r.byRepo, "usage.byRepo", parseShare),
    byConversation: list(r.byConversation, "usage.byConversation", parseShare),
  };
};

export const getAgentUsageSeries = (
  name: string,
  usageRange: UsageRange,
  signal?: AbortSignal,
): Promise<AgentUsageSeries> =>
  request("GET", `/library/agents/${encodeURIComponent(name)}/usage?range=${usageRange}`, parseSeries, undefined, signal);
