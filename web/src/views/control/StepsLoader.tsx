import { useEffect } from "react";
import type { Task } from "../../api/types";
import { useSteps } from "../../data";
import { bookKey, type StepsEntry } from "./traceRows";

export const LIVE_STEPS_MS = 5000;

export interface StepsLoaderProps {
  readonly task: Task;
  readonly live: boolean;
  readonly onSteps: (key: string, entry: StepsEntry) => void;
}

export function StepsLoader({ task, live, onSteps }: StepsLoaderProps) {
  const state = useSteps(task.name, { repo: task.repo, live, intervalMs: LIVE_STEPS_MS });
  const key = bookKey(task.name);
  useEffect(() => onSteps(key, state), [key, state, onSteps]);
  return null;
}
