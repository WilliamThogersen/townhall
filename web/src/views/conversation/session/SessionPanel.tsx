import { useMemo, useRef } from "react";
import type { ConversationDetail, Task } from "../../../api/types";
import { Link, PATHS } from "../../../router";
import { LoopStrip } from "./LoopStrip";
import { ContextSection, FilesSection, RepoSection, TasksEmpty } from "./SessionSections";
import { TaskList } from "./TaskList";
import { taskCount, taskSummary, taskViews } from "./taskRows";
import { useTaskKeys } from "./useTaskKeys";
import "./session.css";

export const SESSION_PANEL_ID = "th-session-panel";

export interface SessionPanelProps {
  readonly detail: ConversationDetail;
  readonly openTask: string | null;
  readonly onOpenTask: (name: string) => void;
}

export function controlCenterPath(conversationId: string): string {
  return `${PATHS.control}?c=${encodeURIComponent(conversationId)}`;
}

function SessionHead({ conversationId, tasks }: { readonly conversationId: string; readonly tasks: readonly Task[] }) {
  return (
    <div className="th-session-head">
      <h3>session</h3>
      <span title={taskSummary(tasks)}>{taskCount(tasks.length)}</span>
      <Link className="th-btn th-btn--ghost th-btn--sm" to={controlCenterPath(conversationId)}>
        control center ↗
      </Link>
    </div>
  );
}

export function SessionPanel({ detail, openTask, onOpenTask }: SessionPanelProps) {
  const { conversation, tasks, attachments } = detail;
  const views = useMemo(() => taskViews(tasks), [tasks]);
  const listRef = useRef<HTMLUListElement>(null);
  useTaskKeys(listRef, tasks.length > 0);

  return (
    <aside className="th-session" id={SESSION_PANEL_ID} aria-label="Session">
      <SessionHead conversationId={conversation.id} tasks={tasks} />
      <div className="th-session-body">
        <LoopStrip tasks={tasks} />
        <h4 className="th-section">tasks {tasks.length}</h4>
        {views.length > 0 ? (
          <TaskList views={views} openTask={openTask} onOpenTask={onOpenTask} listRef={listRef} />
        ) : (
          <TasksEmpty />
        )}
        <h4 className="th-section">repo</h4>
        <RepoSection detail={detail} />
        <h4 className="th-section">files {attachments.length}</h4>
        <FilesSection detail={detail} />
        <h4 className="th-section">context</h4>
        <ContextSection usage={conversation.context} />
      </div>
    </aside>
  );
}
