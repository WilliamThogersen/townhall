import { useState, type RefObject } from "react";
import { agentStateOf } from "../../agents/agentState";
import type { Agent } from "../../api/types";
import { AGENT_STATE_WORDS, StatusGlyph } from "../../ui/StatusGlyph";
import { moveRowFocus } from "./rowFocus";

export interface AgentRowsProps {
  readonly agents: readonly Agent[];
  readonly openName: string | null;
  readonly onOpen: (name: string) => void;
  readonly listRef: RefObject<HTMLUListElement | null>;
}

interface AgentRowProps {
  readonly agent: Agent;
  readonly open: boolean;
  readonly tabStop: boolean;
  readonly onOpen: (name: string) => void;
  readonly onActive: (name: string) => void;
}

export function rowLabel(agent: Agent): string {
  const words = [agent.name, agent.kind, agent.branch, AGENT_STATE_WORDS[agentStateOf(agent)]];
  return words.filter((word) => word).join(" ");
}

function AgentRow({ agent, open, tabStop, onOpen, onActive }: AgentRowProps) {
  const state = agentStateOf(agent);
  const choose = () => {
    onActive(agent.name);
    onOpen(agent.name);
  };
  return (
    <li className="th-span" data-depth="1">
      <button
        type="button"
        className="th-span-label"
        data-agent={agent.name}
        aria-label={rowLabel(agent)}
        aria-current={open ? "true" : undefined}
        tabIndex={tabStop ? 0 : -1}
        onFocus={() => onActive(agent.name)}
        onClick={choose}
      >
        <StatusGlyph state={state} />
        <span className="th-span-name">{agent.name}</span>
        {agent.kind && <span className="th-span-meta">{agent.kind}</span>}
        <span className="th-span-meta">{agent.branch}</span>
      </button>
    </li>
  );
}

function pickTabStop(agents: readonly Agent[], preferred: readonly (string | null)[]): string | null {
  const known = preferred.find((name) => name !== null && agents.some((a) => a.name === name));
  return known ?? agents[0]?.name ?? null;
}

export function AgentRows({ agents, openName, onOpen, listRef }: AgentRowsProps) {
  const [active, setActive] = useState<string | null>(null);
  const tabStop = pickTabStop(agents, [active, openName]);
  return (
    <ul ref={listRef} className="th-list th-control-rows" aria-label="agents" onKeyDown={moveRowFocus}>
      {agents.map((agent) => (
        <AgentRow
          key={agent.name}
          agent={agent}
          open={agent.name === openName}
          tabStop={agent.name === tabStop}
          onOpen={onOpen}
          onActive={setActive}
        />
      ))}
    </ul>
  );
}
