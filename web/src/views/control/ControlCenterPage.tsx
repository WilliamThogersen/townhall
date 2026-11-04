import { useMemo, useRef, useState } from "react";
import { AgentPane } from "../../agents/AgentPane";
import { useAgentsContext } from "../../agents/AgentsProvider";
import { CreateAgentForm } from "../../agents/CreateAgentForm";
import { ControlBody } from "./ControlBody";
import { ControlHead, NEW_AGENT_FORM_ID } from "./ControlHead";
import { findAgent, sortAgents } from "./controlModel";
import { focusAgentRow, focusCurrentRow } from "./rowFocus";
import { useControlKeys } from "./useControlKeys";
import { useOpenAgent } from "./useOpenAgent";
import "./control.css";

function useControlCenter() {
  const { agents, loading, refresh } = useAgentsContext();
  const { openName, open, close } = useOpenAgent();
  const [formOpen, setFormOpen] = useState(false);
  const listRef = useRef<HTMLUListElement>(null);
  const toggleRef = useRef<HTMLButtonElement>(null);
  const sorted = useMemo(() => sortAgents(agents), [agents]);
  const openAgent = findAgent(agents, openName);

  const focusRow = (name: string | null) => {
    const list = listRef.current;
    if (name !== null && focusAgentRow(list, name)) return;
    if (!focusCurrentRow(list)) toggleRef.current?.focus();
  };

  const closePane = () => {
    close();
    focusRow(openName);
  };

  const stopped = async () => {
    close();
    await refresh();
    focusRow(openName);
  };

  const created = async (name: string) => {
    setFormOpen(false);
    open(name);
    await refresh();
    focusRow(name);
  };

  useControlKeys({
    hasRows: sorted.length > 0,
    paneOpen: openAgent !== null,
    onClosePane: closePane,
    onFocusRows: () => focusRow(null),
  });

  return { loading, sorted, openName, openAgent, open, formOpen, setFormOpen, listRef, toggleRef, closePane, stopped, created };
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
          toggleRef={c.toggleRef}
        />
        <div className="th-trace th-control-trace">
          {c.formOpen && <CreateAgentForm id={NEW_AGENT_FORM_ID} onCreated={(name) => void c.created(name)} />}
          <ControlBody loading={c.loading} agents={c.sorted} openName={c.openName} onOpen={c.open} listRef={c.listRef} />
        </div>
      </div>
      {c.openAgent && (
        <div className="th-panes">
          <AgentPane key={c.openAgent.name} agent={c.openAgent} onClose={c.closePane} onStopped={() => void c.stopped()} />
        </div>
      )}
    </div>
  );
}
