import { useState } from "react";
import { AgentCard } from "../agents/AgentCard";
import { useAgentsContext } from "../agents/AgentsProvider";
import { CreateAgentForm } from "../agents/CreateAgentForm";
import { NEW_CONVERSATION_HINT, SEARCH_HINT } from "../shell/globalKeys";
import { useKeyHints } from "../shell/keyHints";

export function LegacyControlCenter() {
  const { agents, loading, refresh } = useAgentsContext();
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());
  useKeyHints([NEW_CONVERSATION_HINT, SEARCH_HINT]);

  const toggle = (name: string) =>
    setOpened((prev) => {
      const next = new Set(prev);
      if (next.has(name)) next.delete(name);
      else next.add(name);
      return next;
    });

  const created = (name: string) => {
    setOpened((prev) => new Set(prev).add(name));
    void refresh();
  };

  return (
    <div className="th-page-body">
      <div className="app">
        <header>
          <h1>townhall</h1>
          <span className="count">{agents.length} agents</span>
        </header>
        <CreateAgentForm onCreated={created} />
        {loading ? (
          <p className="muted">Loading agents...</p>
        ) : agents.length === 0 ? (
          <p className="muted">No agents yet. Create one above.</p>
        ) : (
          <ul className="grid">
            {agents.map((a) => (
              <AgentCard
                key={a.name}
                agent={a}
                open={opened.has(a.name)}
                onToggle={toggle}
                onStopped={() => void refresh()}
              />
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
