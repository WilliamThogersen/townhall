import { createElement, type JSX } from "react";
import { AgentPane } from "../../../agents/AgentPane";
import type { ConversationDetail, Task } from "../../../api/types";

export interface SessionPanelProps {
  readonly detail: ConversationDetail;
  readonly openTask: string | null;
  readonly onOpenTask: (name: string) => void;
}

export interface TaskTerminalProps {
  readonly task: Task;
  readonly onClose: () => void;
}

function taskRow(task: Task, props: SessionPanelProps) {
  const button = createElement(
    "button",
    {
      type: "button",
      className: "th-task-main",
      "aria-pressed": props.openTask === task.name,
      onClick: () => props.onOpenTask(task.name),
    },
    createElement("span", { className: "th-task-name" }, task.name),
  );
  return createElement("li", { key: task.name, className: "th-task" }, button);
}

export function SessionPanel(props: SessionPanelProps): JSX.Element {
  const { tasks } = props.detail;
  const head = createElement(
    "div",
    { className: "th-session-head" },
    createElement("h3", null, "session"),
    createElement("span", null, `${tasks.length} tasks`),
  );
  const list = createElement("ul", { className: "th-tasks-list" }, ...tasks.map((task) => taskRow(task, props)));
  return createElement(
    "aside",
    { className: "th-session", "aria-label": "Session" },
    head,
    createElement("div", { className: "th-session-body" }, list),
  );
}

export function TaskTerminal(props: TaskTerminalProps): JSX.Element {
  const agent = {
    name: props.task.name,
    kind: "claude",
    branch: `townhall/${props.task.name}`,
    worktree: "",
    alive: true,
  };
  return createElement(
    "div",
    { className: "th-panes" },
    createElement(AgentPane, { agent, onClose: props.onClose, onStopped: props.onClose, focused: true }),
  );
}
