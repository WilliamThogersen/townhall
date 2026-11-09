import { listOf, nullable, num, oneOf, record, str, strings, type Guard } from "./guardKit";
import type { Checks, Pr, Task, TaskState, Verify, VerifyPhase } from "./types";

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

const DEFAULT_MAX_RETRIES = 2;

const VERIFY_PHASES = ["verifying", "retrying", "passed", "failed"] as const satisfies readonly VerifyPhase[];

function parseVerify(value: unknown): Verify {
  const r = record(value, "task.verify");
  return {
    phase: oneOf(VERIFY_PHASES)(r.phase, "task.verify.phase"),
    retries: num(r.retries, "task.verify.retries"),
    output: nullable(r.output, "task.verify.output", str),
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
    prBase: nullable(r.prBase, "task.prBase", str),
    doneWhen: nullable(r.doneWhen, "task.doneWhen", str),
    check: nullable(r.check, "task.check", str),
    maxRetries: r.maxRetries === undefined ? DEFAULT_MAX_RETRIES : num(r.maxRetries, "task.maxRetries"),
    verify: nullable(r.verify, "task.verify", parseVerify),
  };
}

export const parseTasks: Guard<readonly Task[]> = listOf(parseTask, "tasks");
