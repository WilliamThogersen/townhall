import type { ReactNode } from "react";
import { Button } from "../ui/Button";
import type { ComposerLook } from "./variants";

export interface ComposerToolsProps {
  readonly look: ComposerLook;
  readonly submitLabel: string;
  readonly help: string;
  readonly error: string | null;
  readonly tools?: ReactNode;
  readonly pluginActions?: ReactNode;
  readonly onAttach: (() => void) | null;
  readonly attachDisabled: boolean;
  readonly onRetry: (() => void) | null;
  readonly onStop: (() => void) | null;
  readonly canSubmit: boolean;
  readonly submitting: boolean;
}

function SubmitButton({ look, submitLabel, onStop, canSubmit, submitting }: ComposerToolsProps) {
  if (onStop !== null) {
    return (
      <Button variant="danger" size="sm" onClick={onStop}>
        stop
      </Button>
    );
  }
  return (
    <Button
      type="submit"
      variant={look.button}
      size="sm"
      disabled={!canSubmit}
      busy={submitting}
      busyLabel={look.busyLabel}
    >
      {submitLabel}
    </Button>
  );
}

export function ComposerTools(props: ComposerToolsProps) {
  const { tools, pluginActions, help, error, onAttach, attachDisabled, onRetry } = props;
  return (
    <div className="th-composer-tools">
      {onAttach && (
        <Button variant="ghost" size="sm" aria-label="Attach files or images" disabled={attachDisabled} onClick={onAttach}>
          + attach
        </Button>
      )}
      {tools}
      {pluginActions}
      <span className="th-help" data-tone={error === null ? undefined : "error"} aria-live="polite">
        {error === null ? help : `error: ${error}`}
      </span>
      {onRetry && (
        <Button variant="ghost" size="sm" onClick={onRetry}>
          retry
        </Button>
      )}
      <SubmitButton {...props} />
    </div>
  );
}
