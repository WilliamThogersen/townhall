import { bool, list, num, oneOf, record, str, type Check, type Guard } from "./guardKit";
import { request } from "./request";

export type PermissionVerdict = "allow" | "deny" | "ask-user" | "hard-deny" | "timeout";

export interface PermissionDecision {
  readonly id: number;
  readonly task: string;
  readonly agentType: string;
  readonly command: string;
  readonly pattern: string;
  readonly verdict: PermissionVerdict;
  readonly reason: string;
  readonly createdAt: string;
}

export interface PermissionSuggestion {
  readonly agentType: string;
  readonly tool: string;
  readonly approvals: number;
}

export interface Permissions {
  readonly decisions: readonly PermissionDecision[];
  readonly suggestions: readonly PermissionSuggestion[];
}

export interface AcceptedSuggestion {
  readonly agentType: string;
  readonly tool: string;
  readonly added: boolean;
}

const verdict = oneOf<PermissionVerdict>(["allow", "deny", "ask-user", "hard-deny", "timeout"]);

const parseDecision: Check<PermissionDecision> = (value, what) => {
  const r = record(value, what);
  return {
    id: num(r.id, `${what}.id`),
    task: str(r.task, `${what}.task`),
    agentType: str(r.agentType, `${what}.agentType`),
    command: str(r.command, `${what}.command`),
    pattern: str(r.pattern, `${what}.pattern`),
    verdict: verdict(r.verdict, `${what}.verdict`),
    reason: str(r.reason, `${what}.reason`),
    createdAt: str(r.createdAt, `${what}.createdAt`),
  };
};

const parseSuggestion: Check<PermissionSuggestion> = (value, what) => {
  const r = record(value, what);
  return {
    agentType: str(r.agentType, `${what}.agentType`),
    tool: str(r.tool, `${what}.tool`),
    approvals: num(r.approvals, `${what}.approvals`),
  };
};

const parsePermissions: Guard<Permissions> = (value) => {
  const r = record(value, "permissions");
  return {
    decisions: list(r.decisions, "permissions.decisions", parseDecision),
    suggestions: list(r.suggestions, "permissions.suggestions", parseSuggestion),
  };
};

const parseAccepted: Guard<AcceptedSuggestion> = (value) => {
  const r = record(value, "accepted suggestion");
  return {
    agentType: str(r.agentType, "accepted.agentType"),
    tool: str(r.tool, "accepted.tool"),
    added: bool(r.added, "accepted.added"),
  };
};

export const getPermissions = (signal?: AbortSignal): Promise<Permissions> =>
  request("GET", "/permissions", parsePermissions, undefined, signal);

export const acceptSuggestion = (agentType: string, tool: string): Promise<AcceptedSuggestion> =>
  request("POST", "/permissions/suggestions/accept", parseAccepted, { agentType, tool });
