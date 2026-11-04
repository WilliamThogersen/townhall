import { listTasks } from "../api/tasks";
import type { Task } from "../api/types";
import { useDataContext } from "./dataContext";
import type { TaskFilter, TasksState } from "./types";
import { useLoad } from "./useLoad";

const NO_TASKS: readonly Task[] = [];

export function useTasks(filter: TaskFilter = {}): TasksState {
  const { refresh } = useDataContext();
  const { conversationId, repo } = filter;
  const { data, loading, error } = useLoad(
    JSON.stringify(["tasks", conversationId ?? null, repo ?? null]),
    (signal) => listTasks({ conversation: conversationId, repo }, signal),
    refresh,
  );
  return { tasks: data ?? NO_TASKS, loading, error };
}
