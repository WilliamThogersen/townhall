import { useTasks } from "../data";
import { useTaskNotifications } from "./useTaskNotifications";

export function TaskNotifier() {
  const { tasks, loading } = useTasks();
  useTaskNotifications(tasks, loading);
  return null;
}
