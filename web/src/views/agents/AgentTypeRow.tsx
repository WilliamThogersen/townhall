import type { AgentTypeSummary } from "../../api/types";
import { AgentAvatar } from "./AgentAvatar";
import { isConversationType, pipelineStep, type Usage } from "./agentTypes";

export interface AgentTypeRowProps {
  readonly type: AgentTypeSummary;
  readonly isDefault: boolean;
  readonly selected: boolean;
  readonly usage: Usage | null;
  readonly onSelect: (name: string) => void;
}

export function StepBadge({ name }: { readonly name: string }) {
  const step = pipelineStep(name);
  return step === null ? null : <span className="th-tag th-tag--ok" title="pipeline step">{step}</span>;
}

export function AgentTypeRow({ type, isDefault, selected, usage, onSelect }: AgentTypeRowProps) {
  return (
    <button type="button" className="th-agent-row" aria-pressed={selected} onClick={() => onSelect(type.name)}>
      <AgentAvatar name={type.name} spec={type.avatar} scale={3} />
      <span className="th-agent-row-body">
        <span className="th-agent-row-head">
          <b>{type.name}</b>
          {isDefault ? <span className="th-tag">task default</span> : null}
          {isConversationType(type.name) ? <span className="th-tag">conversation default</span> : null}
          <StepBadge name={type.name} />
        </span>
        <span className="th-agent-row-desc">{type.description}</span>
        <span className="th-cap th-agent-row-meta">
          <span>{type.adapter}</span>
          {type.model ? <span>{type.model}</span> : null}
          {type.permissionMode ? <span className="th-agent-mode">{type.permissionMode}</span> : null}
          {usage === null ? null : <span>{usage.runs === 1 ? "1 run" : `${usage.runs} runs`}</span>}
        </span>
      </span>
    </button>
  );
}
