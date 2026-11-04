import { useCallback } from "react";
import { addRepo, listRepos, removeRepo } from "../api/repos";
import type { AddRepoInput, Repo } from "../api/types";
import { useDataContext } from "./dataContext";
import type { ReposState } from "./types";
import { useLoad } from "./useLoad";

export function useRepos(): ReposState {
  const { refresh } = useDataContext();
  const { data, loading, error, reload } = useLoad("repos", (signal) => listRepos(signal), refresh);

  const add = useCallback(
    async (input: AddRepoInput): Promise<Repo> => {
      const repo = await addRepo(input);
      reload();
      return repo;
    },
    [reload],
  );

  const remove = useCallback(
    async (name: string): Promise<void> => {
      await removeRepo(name);
      reload();
    },
    [reload],
  );

  return { repos: data, loading, error, add, remove };
}
