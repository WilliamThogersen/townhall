import { useState, type ReactNode } from "react";
import { AgentCard } from "./agents/AgentCard";
import { CreateAgentForm } from "./agents/CreateAgentForm";
import { useAgents } from "./agents/useAgents";
import { useDeathNotifications } from "./agents/useDeathNotifications";

export function App({ toolbar }: { toolbar?: ReactNode }) {
  const { agents, loading, error, refresh } = useAgents();
  useDeathNotifications(agents, loading);
  const [opened, setOpened] = useState<ReadonlySet<string>>(new Set());

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
    <div className="app">
      <header>
        <h1>townhall</h1>
        <span className="count">{agents.length} agents</span>
        {toolbar}
      </header>
      <CreateAgentForm onCreated={created} />
      {error && (
        <p className="banner error" role="alert">
          Cannot refresh agents: {error}
        </p>
      )}
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
  );
}
