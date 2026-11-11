import { request } from "./request";
import { parseSystem } from "./systemGuards";
import type { SystemStats } from "./types";

export const getSystem = (signal?: AbortSignal): Promise<SystemStats> =>
  request("GET", "/system", parseSystem, undefined, signal);
