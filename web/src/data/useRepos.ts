import { useMemo } from "react";
import { useDataStore, useResource } from "./dataContext";
import { REPOS_KEY } from "./resources";
import type { ReposState } from "./types";

export function useRepos(): ReposState {
  const store = useDataStore();
  const { data, loading, error } = useResource(REPOS_KEY, (state) => state.repos);
  return useMemo(
    () => ({ repos: data, loading, error, add: store.addRepo, remove: store.removeRepo, setAccounts: store.setRepoAccounts }),
    [data, loading, error, store],
  );
}
