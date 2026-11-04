import type { Agent } from "../api/types";
import { Button } from "../ui/Button";
import { Spinner } from "../ui/Spinner";
import { StatusGlyph } from "../ui/StatusGlyph";
import { agentStateOf } from "./agentState";
import type { PaneStreamView } from "./paneStream";
import { PaneStopConfirm, useStopConfirm } from "./PaneStop";

interface PaneHeadProps {
  readonly agent: Agent;
  readonly stream: PaneStreamView;
  readonly onClose: () => void;
  readonly onStopped: () => void;
}

function StreamStatus({ view }: { view: PaneStreamView }) {
  return (
    <span className="th-stream" data-status={view.status} role="status">
      {view.glyph === null ? <Spinner /> : <span aria-hidden="true">{view.glyph}</span>}{" "}
      {view.label}
    </span>
  );
}

function AgentMeta({ agent }: { agent: Agent }) {
  return (
    <>
      {agent.kind && (
        <>
          <span className="th-sep" aria-hidden="true">
            ·
          </span>
          <span className="th-pane-meta">{agent.kind}</span>
        </>
      )}
      <span className="th-sep" aria-hidden="true">
        ·
      </span>
      <span className="th-pane-branch" title={agent.branch}>
        {agent.branch}
      </span>
    </>
  );
}

export function PaneHead({ agent, stream, onClose, onStopped }: PaneHeadProps) {
  const stop = useStopConfirm(agent.name, onStopped);
  const state = agentStateOf(agent);

  return (
    <div
      className="th-pane-head"
      data-confirm={stop.confirming ? "true" : undefined}
      onKeyDown={stop.confirming ? stop.onKeyDown : undefined}
    >
      <StatusGlyph state={state} />
      <span className="th-pane-title">{agent.name}</span>
      <span className="th-sr-only">{state}</span>
      {stop.confirming ? (
        <PaneStopConfirm name={agent.name} stop={stop} />
      ) : (
        <>
          <AgentMeta agent={agent} />
          <div className="th-pane-actions">
            <StreamStatus view={stream} />
            <Button variant="ghost" size="sm" onClick={onClose}>
              close
            </Button>
            <Button ref={stop.stopButton} variant="danger" size="sm" onClick={stop.ask}>
              stop
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
