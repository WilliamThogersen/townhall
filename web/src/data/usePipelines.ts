import { getPipeline, listPipelines } from "../api/pipelines";
import type { Pipeline } from "../api/types";
import { useLoad, type Loaded } from "./useLoad";

const NO_REFRESH = 0;

export function usePipelines(): Loaded<readonly Pipeline[]> {
  return useLoad("pipelines", listPipelines, NO_REFRESH);
}

export function usePipeline(name: string | null): Loaded<Pipeline> {
  return useLoad(name === null ? null : `pipeline:${name}`, (signal) => getPipeline(name ?? "", signal), NO_REFRESH);
}
