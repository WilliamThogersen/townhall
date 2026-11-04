import { isDesktop } from "../api/base";

export type DaemonMode = "embedded" | "external" | "stopped";

export interface RepoStatus {
  readonly repo: string | null;
  readonly mode: DaemonMode;
}

/** Calls a Tauri command. Rejects with a readable message outside the desktop shell. */
async function call<T>(cmd: string): Promise<T> {
  if (!isDesktop()) throw new Error("desktop commands are only available in the app");
  const { invoke } = await import("@tauri-apps/api/core");
  return invoke<T>(cmd);
}

export const repoStatus = (): Promise<RepoStatus> => call("repo_status");

/** Opens the native folder picker; the backend validates and restarts the daemon. */
export const chooseRepo = (): Promise<RepoStatus> => call("choose_repo");
