import { ApiError } from "./errors";
import type { Agent } from "./types";

export type Guard<T> = (value: unknown) => T;

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

function fail(what: string): never {
  throw new ApiError("protocol", `unexpected response shape: ${what}`);
}

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
  if (!isRecord(value)) return fail("agent is not an object");
  const { name, kind, branch, worktree, alive } = value;
  if (typeof name !== "string") return fail("agent.name");
  if (kind !== null && kind !== undefined && typeof kind !== "string") return fail("agent.kind");
  if (typeof branch !== "string") return fail("agent.branch");
  if (typeof worktree !== "string") return fail("agent.worktree");
  if (typeof alive !== "boolean") return fail("agent.alive");
  return { name, kind: kind ?? null, branch, worktree, alive };
}

export function parseAgents(value: unknown): readonly Agent[] {
  if (!Array.isArray(value)) return fail("agents is not an array");
  return value.map(parseAgent);
}

export function parseString(value: unknown): string {
  return typeof value === "string" ? value : fail("expected a string");
}
