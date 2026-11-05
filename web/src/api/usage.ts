import { request } from "./request";
import type { ClaudeUsage } from "./types";
import { parseUsage } from "./usageGuards";

export const getUsage = (signal?: AbortSignal): Promise<ClaudeUsage> =>
  request("GET", "/usage", parseUsage, undefined, signal);
