import { useState } from "react";
import { useAgentTypes } from "../../data";
import { Banner } from "../../ui/Banner";
import { DRAG_REGION } from "../../ui/dragRegion";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { AgentTypeDetail } from "./AgentTypeDetail";
import { AgentTypeRow } from "./AgentTypeRow";
import { defaultTypeName } from "./agentTypes";
import "../settings/cap.css";
import "./agents.css";

export function AgentsPage() {
  const { data, loading, error } = useAgentTypes();
  const [selected, setSelected] = useState<string | null>(null);
  const defaultName = data === null ? null : defaultTypeName(data);
  return (
    <div className="th-page">
      <header className="th-page-head" {...DRAG_REGION}>
        <h1>agents</h1>
        {data === null ? null : <span className="th-cap">{data.length}</span>}
      </header>
      <div className="th-page-body">
        {error !== null ? <Banner level="error">{`agent types not loaded: ${error}`}</Banner> : null}
        {data === null && loading ? (
          <div className="th-empty">
            <Loading verb="loading agent types" />
          </div>
        ) : null}
        {data !== null && data.length === 0 ? (
          <EmptyState title="no agent types yet">save one and it shows up here with its TOML</EmptyState>
        ) : null}
        {data !== null && data.length > 0 ? (
          <div className="th-agents">
            <div className="th-agent-list">
              {data.map((type) => (
                <AgentTypeRow
                  key={type.name}
                  type={type}
                  isDefault={type.name === defaultName}
                  selected={type.name === selected}
                  onSelect={setSelected}
                />
              ))}
            </div>
            {selected === null ? (
              <EmptyState title="pick an agent type to see its TOML" />
            ) : (
              <AgentTypeDetail key={selected} name={selected} />
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}
