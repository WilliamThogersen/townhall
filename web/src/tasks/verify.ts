import type { Task } from "../api/types";

function retried(retries: number): string {
  return `${retries} ${retries === 1 ? "retry" : "retries"}`;
}

/**
 * Where a task's done-when check stands, in words: "verifying", "retry 1/2",
 * "check failed after 2 retries", "check passed". `null` for a task without a
 * check, one whose agent has not ended yet, or one that moved on to a pull request.
 */
export function verifyWords(task: Task): string | null {
  const verify = task.verify;
  if (task.check === null || verify === null) return null;
  const retry = `retry ${verify.retries}/${task.maxRetries}`;
  switch (verify.phase) {
    case "verifying":
      return verify.retries > 0 ? `verifying · ${retry}` : "verifying";
    case "retrying":
      return retry;
    case "failed":
      return verify.retries > 0 ? `check failed after ${retried(verify.retries)}` : "check failed";
    case "passed":
      return task.state === "ended" ? "check passed" : null;
  }
}
