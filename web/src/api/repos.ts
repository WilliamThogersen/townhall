import { parseRepo, parseRepos } from "./guards";
import { request } from "./request";
import type { AddRepoInput, Repo } from "./types";

export const listRepos = (signal?: AbortSignal): Promise<readonly Repo[]> =>
  request("GET", "/repos", parseRepos, undefined, signal);

export const addRepo = (input: AddRepoInput): Promise<Repo> =>
  request("POST", "/repos", parseRepo, input);

export const removeRepo = (name: string, deleteClone = false): Promise<Repo> =>
  request("DELETE", `/repos/${encodeURIComponent(name)}${deleteClone ? "?deleteClone=true" : ""}`, parseRepo);
