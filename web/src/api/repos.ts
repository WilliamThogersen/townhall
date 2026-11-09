import { parseRepo, parseRepos } from "./guards";
import { request } from "./request";
import type { AddRepoInput, PatchRepoInput, PutRepoAccountsInput, Repo } from "./types";

export const listRepos = (signal?: AbortSignal): Promise<readonly Repo[]> =>
  request("GET", "/repos", parseRepos, undefined, signal);

export const addRepo = (input: AddRepoInput): Promise<Repo> =>
  request("POST", "/repos", parseRepo, input);

export const patchRepo = (name: string, input: PatchRepoInput): Promise<Repo> =>
  request("PATCH", `/repos/${encodeURIComponent(name)}`, parseRepo, input);

export const removeRepo = (name: string, deleteClone = false): Promise<Repo> =>
  request("DELETE", `/repos/${encodeURIComponent(name)}${deleteClone ? "?deleteClone=true" : ""}`, parseRepo);

export const putRepoAccounts = (name: string, input: PutRepoAccountsInput): Promise<Repo> =>
  request("PUT", `/repos/${encodeURIComponent(name)}/accounts`, parseRepo, input);
