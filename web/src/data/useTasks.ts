import { useMemo } from "react";
import type { Task } from "../api/types";
import { useResource } from "./dataContext";
import { TASKS_KEY } from "./resources";
import type { TaskFilter, TasksState } from "./types";

const NO_TASKS: readonly Task[] = [];

export function matchesFilter(task: Task, filter: TaskFilter): boolean {
  const { conversationId, repo } = filter;
  if (conversationId !== undefined && task.conversationId !== conversationId) return false;
  return repo === undefined || task.repo.toLowerCase() === repo.toLowerCase();
}

export function useTasks(filter: TaskFilter = {}): TasksState {
  const { conversationId, repo } = filter;
  const { data, loading, error } = useResource(TASKS_KEY, (state) => state.tasks);
  const tasks = useMemo(() => {
    if (data === null) return NO_TASKS;
    if (conversationId === undefined && repo === undefined) return data;
    return data.filter((task) => matchesFilter(task, { conversationId, repo }));
  }, [data, conversationId, repo]);
  return useMemo(() => ({ tasks, loading, error }), [tasks, loading, error]);
}
