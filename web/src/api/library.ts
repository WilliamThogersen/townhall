import { fail, listOf, nullable, num, oneOf, record, str, strings, type Guard } from "./guardKit";
import { request } from "./request";
import type { AgentAdapterKind, AgentType, AgentTypeMode, AgentTypeSummary, AgentUsage } from "./types";

const adapter = oneOf<AgentAdapterKind>(["claude", "shell"]);
const mode = oneOf<AgentTypeMode>(["default", "acceptEdits", "plan"]);

function parseSummary(value: unknown): AgentTypeSummary {
  const r = record(value, "agent type");
  return {
    name: str(r.name, "agentType.name"),
    description: str(r.description, "agentType.description"),
    adapter: adapter(r.adapter, "agentType.adapter"),
    model: nullable(r.model, "agentType.model", str),
    permissionMode: nullable(r.permissionMode, "agentType.permissionMode", mode),
    updatedAt: str(r.updatedAt, "agentType.updatedAt"),
  };
}

const parseType: Guard<AgentType> = (value) => {
  const r = record(value, "agent type");
  if (typeof r.source !== "string") return fail("agentType.source");
  return {
    ...parseSummary(r),
    tools: strings(r.tools, "agentType.tools"),
    prompt: nullable(r.prompt, "agentType.prompt", str),
    command: nullable(r.command, "agentType.command", str),
    source: r.source,
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

export const listAgentUsage = (signal?: AbortSignal): Promise<readonly AgentUsage[]> =>
  request("GET", "/library/agents/usage", listOf(parseUsage, "agent usage"), undefined, signal);
