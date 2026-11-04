import { AgentPane } from "../../../agents/AgentPane";
import type { Task } from "../../../api/types";
import { useAgentInfos, useConversation } from "../../../data";
import { Button } from "../../../ui/Button";
import { EmptyState } from "../../../ui/EmptyState";
import { Loading } from "../../../ui/Loading";
import { TaskGlyph } from "./TaskGlyph";
import { taskView } from "./taskRows";
import { findTaskAgent, noTerminalText } from "./terminalModel";
import { useCloseKey } from "./useCloseKey";

export interface TaskTerminalProps {
  readonly task: Task;
  readonly onClose: () => void;
}

interface NoTerminalProps extends TaskTerminalProps {
  readonly siblings: readonly Task[];
  readonly loading: boolean;
}

function NoTerminal({ task, siblings, loading, onClose }: NoTerminalProps) {
  const view = taskView(task, siblings);
  const branch = `townhall/${task.name}`;
  return (
    <article className="th-pane" data-active="true" aria-label={`${task.name} terminal`}>
      <div className="th-pane-head">
        <TaskGlyph state={view.glyph} />
        <span className="th-pane-title">{task.name}</span>
        <span className="th-sr-only">{view.words}</span>
        <span className="th-sep" aria-hidden="true">
          ·
        </span>
        <span className="th-pane-branch" title={branch}>
          {branch}
        </span>
        <div className="th-pane-actions">
          <Button variant="ghost" size="sm" onClick={onClose}>
            close
          </Button>
        </div>
      </div>
      {loading ? (
        <div className="th-empty">
          <Loading verb="loading terminal" />
        </div>
      ) : (
        <EmptyState title={noTerminalText(task, siblings)} />
      )}
    </article>
  );
}

export function TaskTerminal({ task, onClose }: TaskTerminalProps) {
  const { agents, loading } = useAgentInfos();
  const { detail } = useConversation(task.conversationId);
  const agent = findTaskAgent(agents, task);
  useCloseKey(onClose);

  return (
    <div className="th-panes">
      {agent?.alive ? (
        <AgentPane key={agent.name} agent={agent} onClose={onClose} onStopped={onClose} focused />
      ) : (
        <NoTerminal task={task} siblings={detail?.tasks ?? [task]} loading={loading && agent === null} onClose={onClose} />
      )}
    </div>
  );
}
