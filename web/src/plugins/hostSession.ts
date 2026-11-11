import { mintHostToken } from "../api/plugins";
import type { HostSession } from "./bridge";

type Mint = (id: string) => Promise<HostSession>;

const sessions = new Map<string, Promise<HostSession>>();

export function hostSession(id: string, mint: Mint = mintHostToken): Promise<HostSession> {
  const kept = sessions.get(id);
  if (kept) return kept;
  const fresh = mint(id);
  sessions.set(id, fresh);
  fresh.catch(() => forgetHostSession(id));
  return fresh;
}

export function forgetHostSession(id: string): void {
  sessions.delete(id);
}
