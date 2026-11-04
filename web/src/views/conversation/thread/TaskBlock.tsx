import type { Task, ThreadEntry } from "../../../api/types";
import { Button } from "../../../ui/Button";
import { useThread } from "./threadContext";
import { Message, TaskGlyph } from "./ThreadLines";
import { taskBlockHead, taskWords } from "./taskView";

type TasksEntry = Extract<ThreadEntry, { kind: "tasksCreated" }>;

interface TaskLineProps {
  readonly name: string;
  readonly task: Task | null;
  readonly tasks: readonly Task[];
}

function TaskLine({ name, task, tasks }: TaskLineProps) {
  const { openTask, onOpenTask } = useThread();
  const open = openTask === name;
  const title = task?.title ?? "";
  return (
    <div className="th-agent-line" data-open={open || undefined}>
      <TaskGlyph state={task?.state ?? "queued"} />
      <b>{name}</b>
      <span className="th-agent-line-kind">{task ? taskWords(task, tasks) : "queued"}</span>
      <span className="th-agent-line-branch" title={title}>
        {title}
      </span>
      <Button variant="ghost" size="sm" aria-label={`open ${name}`} aria-pressed={open} onClick={() => onOpenTask(name)}>
        open
      </Button>
    </div>
  );
}

export function TaskBlock({ entry }: { readonly entry: TasksEntry }) {
  const { tasks } = useThread();
  const mine = entry.tasks.flatMap((name) => tasks.filter((task) => task.name === name));
  const head = taskBlockHead(mine, entry.tasks.length);
  return (
    <Message at={entry.at}>
      <div className="th-msg-body">
        <div className="th-block" role="group" aria-label={head}>
          <div className="th-block-head">{head}</div>
          {entry.tasks.map((name) => (
            <TaskLine key={name} name={name} task={mine.find((task) => task.name === name) ?? null} tasks={tasks} />
          ))}
        </div>
      </div>
    </Message>
  );
}
