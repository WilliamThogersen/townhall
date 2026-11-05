import { lazy, Suspense, useState } from "react";
import type { Agent } from "../api/types";
import { Loading } from "../ui/Loading";
import type { AttachState } from "./attachSocket";
import { PaneHead } from "./PaneHead";
import { paneStreamView } from "./paneStream";
import "./AgentPane.css";

const Terminal = lazy(() => import("./Terminal").then((m) => ({ default: m.Terminal })));

export interface AgentPaneProps {
  readonly agent: Agent;
  readonly onClose: () => void;
  readonly onStopped: () => void;
  readonly focused?: boolean;
}

const CONNECTING: AttachState = { status: "connecting", error: null };

export function AgentPane({ agent, onClose, onStopped, focused = false }: AgentPaneProps) {
  const [attach, setAttach] = useState<AttachState>(CONNECTING);

  return (
    <article className="th-pane" data-active={focused} aria-label={`${agent.name} terminal`}>
      <PaneHead
        agent={agent}
        stream={paneStreamView(attach, agent.alive)}
        onClose={onClose}
        onStopped={onStopped}
      />
      <div className="th-term">
        <Suspense fallback={<Loading verb="loading terminal" />}>
          <Terminal name={agent.name} alive={agent.alive} focused={focused} onState={setAttach} />
        </Suspense>
      </div>
    </article>
  );
}
