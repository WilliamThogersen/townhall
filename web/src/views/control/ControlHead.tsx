import type { Ref } from "react";
import { Button } from "../../ui/Button";

export const NEW_AGENT_FORM_ID = "th-new-agent-form";

export interface ControlHeadProps {
  readonly count: number | null;
  readonly formOpen: boolean;
  readonly onToggleForm: () => void;
  readonly toggleRef?: Ref<HTMLButtonElement>;
}

function countLabel(count: number): string {
  return count === 1 ? "1 agent" : `${count} agents`;
}

export function ControlHead({ count, formOpen, onToggleForm, toggleRef }: ControlHeadProps) {
  return (
    <header className="th-page-head">
      <h1>control center</h1>
      {count !== null && <span className="th-control-cap">{countLabel(count)}</span>}
      <div className="th-page-head-actions">
        <Button
          ref={toggleRef}
          variant="ghost"
          size="sm"
          aria-expanded={formOpen}
          aria-controls={formOpen ? NEW_AGENT_FORM_ID : undefined}
          onClick={onToggleForm}
        >
          new agent…
        </Button>
      </div>
    </header>
  );
}
