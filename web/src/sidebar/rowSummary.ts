import type { Conversation, Task, TaskState } from "../api/types";
import type { AgentState } from "../ui/StatusGlyph";

export type RowState = "waiting" | "error" | "ended" | null;

export interface RowSummary {
  readonly glyph: AgentState;
  readonly state: RowState;
  readonly status: string;
  readonly needsYou: boolean;
}

const RUNNING: ReadonlySet<TaskState> = new Set(["queued", "working", "inCi", "inReview"]);

const countOf = (tasks: readonly Task[], keep: (task: Task) => boolean) => tasks.filter(keep).length;

function about(tasks: readonly Task[], one: string, many: string): string {
  const [first] = tasks;
  if (tasks.length === 1 && first) return `${first.name} ${one}`;
  return `${tasks.length} tasks ${many}`;
}

function progress(conversation: Conversation, tasks: readonly Task[]): string {
  const running = countOf(tasks, (task) => RUNNING.has(task.state));
  const merged = countOf(tasks, (task) => task.state === "merged");
  return [
    conversation.ended ? "ended" : "your turn",
    running > 0 ? `${running} running` : null,
    merged > 0 ? `${merged} merged` : null,
  ]
    .filter((part) => part !== null)
    .join(" · ");
}

function quietGlyph(conversation: Conversation, tasks: readonly Task[]): AgentState {
  if (conversation.status === "thinking") return "working";
  return tasks.some((task) => RUNNING.has(task.state)) ? "alive" : "dead";
}

export function summarizeRow(conversation: Conversation, tasks: readonly Task[]): RowSummary {
  const waiting = tasks.filter((task) => task.state === "needsYou");
  if (waiting.length > 0) {
    return { glyph: "waiting", state: "waiting", status: about(waiting, "needs you", "need you"), needsYou: true };
  }
  const failed = tasks.filter((task) => task.state === "error");
  if (failed.length > 0) {
    return { glyph: "error", state: "error", status: about(failed, "failed", "failed"), needsYou: false };
  }
  const status = conversation.status === "thinking" ? "thinking" : progress(conversation, tasks);
  const state: RowState = conversation.ended ? "ended" : null;
  return { glyph: quietGlyph(conversation, tasks), state, status, needsYou: false };
}
