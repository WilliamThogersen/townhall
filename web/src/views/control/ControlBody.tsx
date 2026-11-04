import type { RefObject } from "react";
import type { Agent } from "../../api/types";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { AgentRows } from "./AgentRows";
import { controlStats } from "./controlModel";
import { ControlStats } from "./ControlStats";

export interface ControlBodyProps {
  readonly loading: boolean;
  readonly agents: readonly Agent[];
  readonly openName: string | null;
  readonly onOpen: (name: string) => void;
  readonly listRef: RefObject<HTMLUListElement | null>;
}

export function ControlBody({ loading, agents, openName, onOpen, listRef }: ControlBodyProps) {
  if (agents.length === 0) {
    return (
      <div className="th-trace-body">
        {loading ? (
          <div className="th-empty">
            <Loading verb="loading agents" />
          </div>
        ) : (
          <EmptyState title="no agents yet">start one with new agent…</EmptyState>
        )}
      </div>
    );
  }
  return (
    <>
      <ControlStats stats={controlStats(agents)} />
      <div className="th-trace-body">
        <AgentRows agents={agents} openName={openName} onOpen={onOpen} listRef={listRef} />
      </div>
    </>
  );
}
