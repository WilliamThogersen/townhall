export type Adapter = "claude" | "shell";

export interface Agent {
  readonly name: string;
  readonly kind: string | null;
  readonly branch: string;
  readonly worktree: string;
  readonly alive: boolean;
}

export interface CreateAgentInput {
  readonly name: string;
  readonly adapter: Adapter;
  readonly base?: string;
  readonly prompt?: string;
  readonly command?: string;
}

export interface Info {
  readonly home: string;
  readonly repo: string;
  readonly repoKey: string;
}
