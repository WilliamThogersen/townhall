import { useState, type RefObject } from "react";
import type { Pr } from "../../../api/types";
import { TaskGlyph } from "./TaskGlyph";
import { sameTask, type TaskView } from "./taskRows";
import { onTaskListKeyDown } from "./useTaskKeys";

export interface TaskListProps {
  readonly views: readonly TaskView[];
  readonly openTask: string | null;
  readonly onOpenTask: (name: string) => void;
  readonly listRef: RefObject<HTMLUListElement | null>;
}

interface TaskRowProps {
  readonly view: TaskView;
  readonly open: boolean;
  readonly tabbable: boolean;
  readonly onOpen: (name: string) => void;
  readonly onFocus: (name: string) => void;
}

function tabStopOf(views: readonly TaskView[], current: string | null, openTask: string | null): string | null {
  const names = views.map((view) => view.task.name);
  return names.find((name) => sameTask(current, name)) ?? names.find((name) => sameTask(openTask, name)) ?? names[0] ?? null;
}

function PrLink({ pr, tabbable }: { readonly pr: Pr | null; readonly tabbable: boolean }) {
  if (pr === null) return <span className="th-task-link" />;
  if (pr.url === null) return <span className="th-task-link">#{pr.number}</span>;
  return (
    <a className="th-task-link" href={pr.url} target="_blank" rel="noreferrer" tabIndex={tabbable ? 0 : -1}>
      #{pr.number}
    </a>
  );
}

function TaskRow({ view, open, tabbable, onOpen, onFocus }: TaskRowProps) {
  const { task } = view;
  return (
    <li className="th-task" data-state={view.tone ?? undefined}>
      <button
        type="button"
        className="th-task-main"
        aria-pressed={open}
        tabIndex={tabbable ? 0 : -1}
        data-task={task.name}
        onClick={() => onOpen(task.name)}
        onFocus={() => onFocus(task.name)}
      >
        <TaskGlyph state={view.glyph} />
        <span className="th-task-name">{task.name}</span>
        <span className="th-task-state">{view.words}</span>
        <span className="th-task-sub">
          <span className="th-task-title">{task.title}</span>
          {view.prOpen && <span className="th-tag">pr open</span>}
        </span>
      </button>
      <PrLink pr={task.pr} tabbable={tabbable} />
    </li>
  );
}

export function TaskList({ views, openTask, onOpenTask, listRef }: TaskListProps) {
  const [current, setCurrent] = useState<string | null>(null);
  const tabStop = tabStopOf(views, current, openTask);
  return (
    <ul className="th-tasks-list" ref={listRef} onKeyDown={onTaskListKeyDown}>
      {views.map((view) => (
        <TaskRow
          key={view.task.name}
          view={view}
          open={sameTask(openTask, view.task.name)}
          tabbable={view.task.name === tabStop}
          onOpen={onOpenTask}
          onFocus={setCurrent}
        />
      ))}
    </ul>
  );
}
