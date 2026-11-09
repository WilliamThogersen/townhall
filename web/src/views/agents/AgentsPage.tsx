import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { useAgentTypes, useAgentUsage } from "../../data";
import { Banner } from "../../ui/Banner";
import { DRAG_REGION } from "../../ui/dragRegion";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { AgentTypeDetail } from "./AgentTypeDetail";
import { AgentTypeRow } from "./AgentTypeRow";
import { defaultTypeName, filterTypes, sortTypes, usageOf } from "./agentTypes";
import { dropLegacyAvatarPrefs } from "./legacyAvatarPrefs";
import "../settings/cap.css";
import "./agents.css";

const ROW_STEPS: Readonly<Record<string, number>> = { ArrowDown: 1, ArrowUp: -1 };

export function AgentsPage() {
  const { data, loading, error, reload } = useAgentTypes();
  const usage = useAgentUsage().data;
  useEffect(dropLegacyAvatarPrefs, []);
  const [selected, setSelected] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const listRef = useRef<HTMLDivElement>(null);
  const defaultName = data === null ? null : defaultTypeName(data);
  const visible = useMemo(
    () => (data === null ? [] : filterTypes(sortTypes(data, defaultName, usage), query)),
    [data, defaultName, usage, query],
  );
  const rows = () => Array.from(listRef.current?.querySelectorAll<HTMLButtonElement>(".th-agent-row") ?? []);

  const onListKey = (event: KeyboardEvent<HTMLDivElement>) => {
    const step = ROW_STEPS[event.key];
    const here = rows().indexOf(event.target as HTMLButtonElement);
    if (step === undefined || here < 0 || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const next = visible[here + step];
    event.preventDefault();
    if (next === undefined) return;
    setSelected(next.name);
    rows()[here + step]?.focus();
  };

  const onFilterKey = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape" && query !== "") {
      event.preventDefault();
      setQuery("");
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      rows()[0]?.focus();
    }
  };

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
            <div className="th-agent-column">
              <input
                type="search"
                className="th-input th-agent-filter"
                placeholder="filter by name, description, model…"
                aria-label="filter agent types"
                value={query}
                onChange={(event) => setQuery(event.target.value)}
                onKeyDown={onFilterKey}
              />
              {visible.length === 0 ? (
                <EmptyState title={`no agent types match "${query.trim()}"`}>clear the filter to see all {data.length}</EmptyState>
              ) : (
                <div className="th-agent-list" ref={listRef} onKeyDown={onListKey}>
                  {visible.map((type) => (
                    <AgentTypeRow
                      key={type.name}
                      type={type}
                      isDefault={type.name === defaultName}
                      selected={type.name === selected}
                      usage={usageOf(usage, type.name)}
                      onSelect={setSelected}
                    />
                  ))}
                </div>
              )}
            </div>
            {selected === null ? (
              <EmptyState title="pick an agent type to see its TOML" />
            ) : (
              <AgentTypeDetail key={selected} name={selected} onSaved={reload} />
            )}
          </div>
        ) : null}
      </div>
    </div>
  );
}
