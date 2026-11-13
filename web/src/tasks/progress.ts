import type { NotifyWhen, Task } from "../api/types";

export const RECENT_MS = 24 * 60 * 60 * 1000;

export type EventKind = "landed" | "failed" | "needsYou";

export interface TaskEvent {
  readonly kind: EventKind;
  readonly task: Task;
}

export interface NotificationText {
  readonly title: string;
  readonly body: string;
  readonly conversationId: string | null;
}

export interface Overview {
  readonly running: number;
  readonly waiting: number;
  readonly landed: number;
}

const KIND_OF_STATE: Partial<Record<Task["state"], EventKind>> = {
  merged: "landed",
  error: "failed",
  needsYou: "needsYou",
};

const ORDER: readonly EventKind[] = ["landed", "failed", "needsYou"];

export function taskEvents(prev: readonly Task[], next: readonly Task[]): TaskEvent[] {
  const before = new Map(prev.map((t) => [t.name, t.state]));
  return next.flatMap((task) => {
    const kind = KIND_OF_STATE[task.state];
    const was = before.get(task.name);
    return kind && was !== undefined && was !== task.state ? [{ kind, task }] : [];
  });
}

export function allowedKinds(when: NotifyWhen): readonly EventKind[] {
  switch (when) {
    case "off":
      return [];
    case "needsYou":
      return ["needsYou"];
    case "failed":
      return ["needsYou", "failed"];
    case "everything":
      return ["needsYou", "failed", "landed"];
  }
}

function single({ kind, task }: TaskEvent): NotificationText {
  const conversationId = task.conversationId;
  switch (kind) {
    case "landed": {
      const pr = task.pr ? `PR #${task.pr.number}` : "it";
      return { title: `landed: ${task.title}`, body: `${pr} is on main. nothing more to do.`, conversationId };
    }
    case "failed":
      return { title: `failed: ${task.title}`, body: "it failed. open the conversation to decide.", conversationId };
    case "needsYou":
      return { title: `needs you: ${task.title}`, body: "it needs your answer.", conversationId };
  }
}

const LABEL: Record<EventKind, string> = { landed: "landed", failed: "failed", needsYou: "needs you" };

export function describeEvents(events: readonly TaskEvent[]): NotificationText {
  const [only] = events;
  if (events.length === 1 && only) return single(only);
  const parts = ORDER.flatMap((kind) => {
    const count = events.filter((e) => e.kind === kind).length;
    return count > 0 ? [`${count} ${LABEL[kind]}`] : [];
  });
  const conversations = new Set(events.map((e) => e.task.conversationId));
  const [one] = conversations;
  return {
    title: `townhall: ${parts.join(", ")}`,
    body: "open the conversation.",
    conversationId: conversations.size === 1 && one !== undefined ? one : null,
  };
}

export function overview(tasks: readonly Task[], now: number): Overview {
  const running = tasks.filter((t) => t.state === "working" || t.state === "inCi" || t.state === "inReview").length;
  const waiting = tasks.filter((t) => t.state === "needsYou").length;
  const landed = tasks.filter((t) => t.state === "merged" && now - Date.parse(t.createdAt) <= RECENT_MS).length;
  return { running, waiting, landed };
}
