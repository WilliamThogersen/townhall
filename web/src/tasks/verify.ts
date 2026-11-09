import type { Task } from "../api/types";

function retried(retries: number): string {
  return `${retries} ${retries === 1 ? "retry" : "retries"}`;
}

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
