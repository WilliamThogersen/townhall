import { withQuery } from "./query";
import { request } from "./request";
import { parseTask, parseTasks } from "./taskGuards";
import type { Task, TaskState } from "./types";

export interface TaskQuery {
  readonly conversation?: string;
  readonly repo?: string;
  readonly state?: TaskState;
}

export const listTasks = (filter: TaskQuery = {}, signal?: AbortSignal): Promise<readonly Task[]> =>
  request(
    "GET",
    withQuery("/tasks", { conversation: filter.conversation, repo: filter.repo, state: filter.state }),
    parseTasks,
    undefined,
    signal,
  );

export const getTask = (name: string, signal?: AbortSignal): Promise<Task> =>
  request("GET", `/tasks/${encodeURIComponent(name)}`, parseTask, undefined, signal);
