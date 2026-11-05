import type { AgentTypeSummary } from "../../api/types";
import { pipelineStep } from "./agentTypes";

export interface AgentTypeRowProps {
  readonly type: AgentTypeSummary;
  readonly isDefault: boolean;
  readonly selected: boolean;
  readonly onSelect: (name: string) => void;
}

export function StepBadge({ name }: { readonly name: string }) {
  const step = pipelineStep(name);
  return step === null ? null : <span className="th-tag th-tag--ok" title="pipeline step">{step}</span>;
}

export function AgentTypeRow({ type, isDefault, selected, onSelect }: AgentTypeRowProps) {
  return (
    <button type="button" className="th-agent-row" aria-pressed={selected} onClick={() => onSelect(type.name)}>
      <span className="th-agent-row-head">
        <b>{type.name}</b>
        {isDefault ? <span className="th-tag">default</span> : null}
        <StepBadge name={type.name} />
      </span>
      <span className="th-agent-row-desc">{type.description}</span>
      <span className="th-cap th-agent-row-meta">
        <span>{type.adapter}</span>
        {type.model ? <span>{type.model}</span> : null}
        {type.permissionMode ? <span className="th-agent-mode">{type.permissionMode}</span> : null}
      </span>
    </button>
  );
}
