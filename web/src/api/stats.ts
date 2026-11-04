import { withQuery } from "./query";
import { request } from "./request";
import { parseStats } from "./statsGuards";
import type { Stats } from "./types";

export const getStats = (repo?: string, signal?: AbortSignal): Promise<Stats> =>
  request("GET", withQuery("/stats", { repo }), parseStats, undefined, signal);
