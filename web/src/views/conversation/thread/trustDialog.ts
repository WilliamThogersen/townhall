import { sendKey } from "../../../api/client";
import type { AgentKey } from "../../../api/types";

export const TRUST_QUESTION = "claude asks whether to trust this worktree";
export const KEY_GAP_MS = 200;

const TRUST_KEYS: ReadonlyMap<string, readonly AgentKey[]> = new Map<string, readonly AgentKey[]>([
  ["yes, trust it", ["up", "enter"]],
  ["no, exit", ["enter"]],
]);

export const takesKeys = (question: string): boolean => question === TRUST_QUESTION;

export function choiceKeys(question: string, choice: string): readonly AgentKey[] | null {
  if (!takesKeys(question)) return null;
  return TRUST_KEYS.get(choice) ?? null;
}

const pause = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));

export async function pressKeys(agent: string, keys: readonly AgentKey[], gapMs = KEY_GAP_MS): Promise<void> {
  for (const [index, key] of keys.entries()) {
    if (index > 0) await pause(gapMs);
    await sendKey(agent, key);
  }
}
