import type { Agent } from "../api/types";
import { AgentPane } from "./AgentPane";
import "./AgentCard.css";

interface Props {
  readonly agent: Agent;
  readonly open: boolean;
  readonly onToggle: (name: string) => void;
  readonly onStopped: () => void;
}

export function AgentCard({ agent, open, onToggle, onStopped }: Props) {
  const toggle = () => onToggle(agent.name);

  if (open) {
    return (
      <li className="card open">
        <div className="card-pane">
          <AgentPane agent={agent} onClose={toggle} onStopped={onStopped} />
        </div>
      </li>
    );
  }

  return (
    <li className="card">
      <div className="card-head">
        <span className={`dot ${agent.alive ? "alive" : "dead"}`} aria-hidden="true" />
        <div className="card-title">
          <strong>{agent.name}</strong>
          <code>{agent.branch}</code>
        </div>
        <span className="badge">{agent.alive ? "alive" : "dead"}</span>
        {agent.kind && <span className="badge muted">{agent.kind}</span>}
        <button onClick={toggle}>Open</button>
      </div>
    </li>
  );
}
