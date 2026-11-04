import type { Agent } from "../api/types";
import { StopButton } from "./StopButton";
import { TerminalPanel } from "./TerminalPanel";

interface Props {
  readonly agent: Agent;
  readonly open: boolean;
  readonly onToggle: (name: string) => void;
  readonly onStopped: () => void;
}

export function AgentCard({ agent, open, onToggle, onStopped }: Props) {
  return (
    <li className={`card${open ? " open" : ""}`}>
      <div className="card-head">
        <span className={`dot ${agent.alive ? "alive" : "dead"}`} aria-hidden="true" />
        <div className="card-title">
          <strong>{agent.name}</strong>
          <code>{agent.branch}</code>
        </div>
        <span className="badge">{agent.alive ? "alive" : "dead"}</span>
        {agent.kind && <span className="badge muted">{agent.kind}</span>}
        <button onClick={() => onToggle(agent.name)} aria-expanded={open}>
          {open ? "Close" : "Open"}
        </button>
        <StopButton name={agent.name} onStopped={onStopped} />
      </div>
      {open && <TerminalPanel name={agent.name} />}
    </li>
  );
}
