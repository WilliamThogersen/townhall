import { withQuery } from "./query";
import { request } from "./request";
import { parseSteps } from "./statsGuards";
import type { Step } from "./types";

export const getSteps = (agent: string, repo?: string, signal?: AbortSignal): Promise<readonly Step[]> =>
  request(
    "GET",
    withQuery(`/agents/${encodeURIComponent(agent)}/steps`, { repo }),
    parseSteps,
    undefined,
    signal,
  );
