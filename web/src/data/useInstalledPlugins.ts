import { listPlugins } from "../api/plugins";
import type { Plugin } from "../api/types";
import { useLoad, type Loaded } from "./useLoad";

const NO_REFRESH = 0;

export function useInstalledPlugins(): Loaded<readonly Plugin[]> {
  return useLoad(
    "installed-plugins",
    async (signal) => (await listPlugins(signal)).installed,
    NO_REFRESH,
  );
}
