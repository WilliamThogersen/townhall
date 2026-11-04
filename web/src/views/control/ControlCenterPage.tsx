import { useMemo, useState } from "react";
import { AgentPane } from "../../agents/AgentPane";
import { useAgentsContext } from "../../agents/AgentsProvider";
import { CreateAgentForm } from "../../agents/CreateAgentForm";
import { ControlBody } from "./ControlBody";
import { ControlHead, NEW_AGENT_FORM_ID } from "./ControlHead";
import { findAgent, sortAgents } from "./controlModel";
import { useControlKeys } from "./useControlKeys";
import { useOpenAgent } from "./useOpenAgent";
import { useRowFocus } from "./useRowFocus";
import "./control.css";

function useControlCenter() {
  const { agents, loading, refresh } = useAgentsContext();
  const { openName, open, close } = useOpenAgent();
  const [formOpen, setFormOpen] = useState(false);
  const focus = useRowFocus();
  const sorted = useMemo(() => sortAgents(agents), [agents]);
  const openAgent = findAgent(agents, openName);

  const closePane = () => {
    close();
    focus.focusRow(openName);
  };

  const stopped = async () => {
    close();
    await refresh();
    focus.focusRowAfterRender(openName);
  };

  const created = async (name: string) => {
    setFormOpen(false);
    open(name);
    await refresh();
    focus.focusRowAfterRender(name);
  };

  useControlKeys({
    hasRows: sorted.length > 0,
    paneOpen: openAgent !== null,
    onClosePane: closePane,
    onFocusRows: () => focus.focusRow(null),
  });

  return { loading, sorted, openName, openAgent, open, formOpen, setFormOpen, focus, closePane, stopped, created };
}

export function ControlCenterPage() {
  const c = useControlCenter();
  const paneOpen = c.openAgent !== null;
  return (
    <div className={paneOpen ? "th-control th-convo-layout" : "th-control"} data-pane={paneOpen ? "true" : undefined}>
      <div className="th-page th-convo-view">
        <ControlHead
          count={c.loading && c.sorted.length === 0 ? null : c.sorted.length}
          formOpen={c.formOpen}
          onToggleForm={() => c.setFormOpen((open) => !open)}
          toggleRef={c.focus.fallbackRef}
        />
        <div className="th-trace th-control-trace">
          {c.formOpen && <CreateAgentForm id={NEW_AGENT_FORM_ID} onCreated={(name) => void c.created(name)} />}
          <ControlBody
            loading={c.loading}
            agents={c.sorted}
            openName={c.openName}
            onOpen={c.open}
            listRef={c.focus.listRef}
          />
        </div>
      </div>
      {c.openAgent && (
        <AgentPane
          key={c.openAgent.name}
          agent={c.openAgent}
          onClose={c.closePane}
          onStopped={() => void c.stopped()}
        />
      )}
    </div>
  );
}
