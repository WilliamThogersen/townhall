import { listOf, nullable, num, oneOf, record, str, strings, type Guard } from "./guardKit";
import type { Checks, Pr, Task, TaskState } from "./types";

const TASK_STATES = [
  "queued",
  "working",
  "needsYou",
  "inCi",
  "inReview",
  "merged",
  "ended",
  "error",
] as const satisfies readonly TaskState[];

const CHECKS = ["pending", "passed", "failed", "none"] as const satisfies readonly Checks[];

export function parsePr(value: unknown): Pr {
  const r = record(value, "task.pr");
  return {
    number: num(r.number, "task.pr.number"),
    url: nullable(r.url, "task.pr.url", str),
    checks: oneOf(CHECKS)(r.checks, "task.pr.checks"),
  };
}

export function parseTask(value: unknown): Task {
  const r = record(value, "task");
  return {
    name: str(r.name, "task.name"),
    title: str(r.title, "task.title"),
    prompt: str(r.prompt, "task.prompt"),
    repo: str(r.repo, "task.repo"),
    baseBranch: str(r.baseBranch, "task.baseBranch"),
    after: strings(r.after, "task.after"),
    state: oneOf(TASK_STATES)(r.state, "task.state"),
    conversationId: str(r.conversationId, "task.conversationId"),
    attachments: strings(r.attachments, "task.attachments"),
    createdAt: str(r.createdAt, "task.createdAt"),
    pr: nullable(r.pr, "task.pr", parsePr),
    agent: str(r.agent, "task.agent"),
    blocked: nullable(r.blocked, "task.blocked", str),
  };
}

export const parseTasks: Guard<readonly Task[]> = listOf(parseTask, "tasks");
