import { bool, fail, isRecord, listOf, nullable, num, record, str, type Guard } from "./guardKit";
import type { Agent, AgentInfo, AgentState, Info, Repo } from "./types";

export type { Guard } from "./guardKit";

export interface Envelope {
  readonly success: boolean;
  readonly data: unknown;
  readonly error: string | null;
}

export function parseEnvelope(value: unknown): Envelope {
  if (!isRecord(value)) return fail("envelope is not an object");
  const { success, data, error } = value;
  if (typeof success !== "boolean") return fail("envelope.success");
  if (error !== null && error !== undefined && typeof error !== "string") {
    return fail("envelope.error");
  }
  return { success, data: data ?? null, error: error ?? null };
}

export function parseAgent(value: unknown): Agent {
  const r = record(value, "agent");
  return {
    name: str(r.name, "agent.name"),
    kind: nullable(r.kind, "agent.kind", str),
    branch: str(r.branch, "agent.branch"),
    worktree: str(r.worktree, "agent.worktree"),
    alive: bool(r.alive, "agent.alive"),
  };
}

export const parseAgents: Guard<readonly Agent[]> = listOf(parseAgent, "agents");

export function parseAgentState(value: unknown): AgentState {
  const r = record(value, "agent.state");
  switch (r.kind) {
    case "working":
      return { kind: "working" };
    case "waiting":
      return { kind: "waiting", question: str(r.question, "agent.state.question") };
    case "error":
      return { kind: "error", exitCode: nullable(r.exitCode, "agent.state.exitCode", num) };
    case "ended":
      return { kind: "ended" };
    default:
      return fail("agent.state.kind");
  }
}

export function parseAgentInfo(value: unknown): AgentInfo {
  const r = record(value, "agent");
  return {
    ...parseAgent(r),
    repo: str(r.repo, "agent.repo"),
    state: parseAgentState(r.state),
  };
}

export const parseAgentInfos: Guard<readonly AgentInfo[]> = listOf(parseAgentInfo, "agents");

export function parseString(value: unknown): string {
  return typeof value === "string" ? value : fail("expected a string");
}

export function parseInfo(value: unknown): Info {
  const r = record(value, "info");
  return {
    home: str(r.home, "info.home"),
    repo: str(r.repo, "info.repo"),
    repoKey: str(r.repoKey, "info.repoKey"),
    userName: nullable(r.userName, "info.userName", str),
  };
}

export function parseRepo(value: unknown): Repo {
  const r = record(value, "repo");
  return {
    name: str(r.name, "repo.name"),
    path: str(r.path, "repo.path"),
    baseBranch: str(r.baseBranch, "repo.baseBranch"),
  };
}

export const parseRepos: Guard<readonly Repo[]> = listOf(parseRepo, "repos");
