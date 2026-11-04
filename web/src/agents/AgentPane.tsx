import { lazy, Suspense } from "react";
import type { Agent } from "../api/types";
import { Loading } from "../ui/Loading";
import { PaneHead } from "./PaneHead";
import { PanePrompt } from "./PanePrompt";
import { paneStreamView } from "./paneStream";
import { useAgentStream } from "./useAgentStream";
import "./AgentPane.css";

const Terminal = lazy(() => import("./Terminal").then((m) => ({ default: m.Terminal })));

export interface AgentPaneProps {
  readonly agent: Agent;
  readonly onClose: () => void;
  readonly onStopped: () => void;
  readonly focused?: boolean;
}

export function AgentPane({ agent, onClose, onStopped, focused = false }: AgentPaneProps) {
  const stream = useAgentStream(agent.name, agent.alive);

  return (
    <article className="th-pane" data-active={focused} aria-label={`${agent.name} terminal`}>
      <PaneHead
        agent={agent}
        stream={paneStreamView(stream, agent.alive)}
        onClose={onClose}
        onStopped={onStopped}
      />
      <div className="th-term">
        <Suspense fallback={<Loading verb="loading terminal" />}>
          <Terminal snapshot={stream.snapshot} />
        </Suspense>
      </div>
      <PanePrompt name={agent.name} />
    </article>
  );
}
