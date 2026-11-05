import type { AgentInfo, Task } from "../../../api/types";
import { pendingAfter, taskView } from "./taskRows";

const sameName = (a: string, b: string) => a.toLowerCase() === b.toLowerCase();

function spokenList(names: readonly string[]): string {
  if (names.length < 2) return names.join("");
  return `${names.slice(0, -1).join(", ")} and ${names.at(-1)}`;
}

function queuedText(task: Task, siblings: readonly Task[]): string {
  const waiting = pendingAfter(task, siblings);
  return waiting.length > 0 ? `queued: waits for ${spokenList(waiting)}` : "queued: waits for a free slot";
}

function withPr(words: string, task: Task): string {
  return task.pr ? `${words}: #${task.pr.number}` : words;
}

export function noTerminalText(task: Task, siblings: readonly Task[]): string {
  const words = taskView(task, siblings).words;
  switch (task.state) {
    case "queued":
      return queuedText(task, siblings);
    case "merged":
    case "inCi":
    case "inReview":
      return withPr(task.state === "inCi" ? "in ci" : words, task);
    case "ended":
    case "error":
      return `${words}: no terminal left`;
    default:
      return `${words}: no terminal yet`;
  }
}

export function findTaskAgent(agents: readonly AgentInfo[], task: Task): AgentInfo | null {
  const named = agents.filter((agent) => sameName(agent.name, task.name));
  return named.find((agent) => sameName(agent.repo, task.repo)) ?? named[0] ?? null;
}
