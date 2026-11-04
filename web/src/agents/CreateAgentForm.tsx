import { useId, useState, type FormEvent } from "react";
import { createAgent } from "../api/client";
import { toMessage } from "../api/errors";
import type { Adapter } from "../api/types";
import { Button } from "../ui/Button";
import { Segmented } from "../ui/Segmented";
import { EMPTY_FORM, validateForm, type FormValues } from "./validation";
import "./CreateAgentForm.css";

const ADAPTERS = [
  { value: "claude", label: "claude" },
  { value: "shell", label: "shell" },
] as const;

interface TextFieldProps {
  readonly label: string;
  readonly value: string;
  readonly onChange: (value: string) => void;
  readonly error?: string;
  readonly placeholder?: string;
  readonly wide?: boolean;
  readonly autoFocus?: boolean;
}

function TextField({ label, value, onChange, error, placeholder, wide, autoFocus }: TextFieldProps) {
  const id = useId();
  const errorId = `${id}-error`;
  return (
    <div className={wide ? "th-new-agent-field th-new-agent-field--wide" : "th-new-agent-field"}>
      <label className="th-new-agent-label" htmlFor={id}>
        {label}
      </label>
      <input
        id={id}
        className="th-input"
        value={value}
        placeholder={placeholder}
        autoFocus={autoFocus}
        autoComplete="off"
        spellCheck={false}
        aria-invalid={error !== undefined}
        aria-describedby={error ? errorId : undefined}
        onChange={(e) => onChange(e.target.value)}
      />
      {error && (
        <span id={errorId} className="th-field-error" role="alert">
          error: {error}
        </span>
      )}
    </div>
  );
}

function AdapterField({ value, onChange }: { value: Adapter; onChange: (next: Adapter) => void }) {
  const name = useId();
  return (
    <div className="th-new-agent-field th-new-agent-field--fit">
      <span className="th-new-agent-label" aria-hidden="true">
        adapter
      </span>
      <Segmented<Adapter> name={name} label="adapter" value={value} options={ADAPTERS} onChange={onChange} />
    </div>
  );
}

function useCreateAgent(onCreated: (name: string) => void) {
  const [values, setValues] = useState<FormValues>(EMPTY_FORM);
  const [touched, setTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { errors, input } = validateForm(values);

  const set = (patch: Partial<FormValues>) => setValues((v) => ({ ...v, ...patch }));
  const shown = touched ? errors : {};

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    setTouched(true);
    if (!input) return;
    setBusy(true);
    setError(null);
    try {
      const agent = await createAgent(input);
      setValues(EMPTY_FORM);
      setTouched(false);
      onCreated(agent.name);
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return { values, set, shown, busy, error, submit };
}

export interface CreateAgentFormProps {
  readonly id?: string;
  readonly onCreated: (name: string) => void;
}

export function CreateAgentForm({ id, onCreated }: CreateAgentFormProps) {
  const { values, set, shown, busy, error, submit } = useCreateAgent(onCreated);
  const shell = values.adapter === "shell";

  return (
    <form id={id} className="th-new-agent" aria-label="new agent" onSubmit={(e) => void submit(e)} noValidate>
      <TextField label="name" value={values.name} onChange={(name) => set({ name })}
        error={shown.name} placeholder="fix-login" autoFocus />
      <AdapterField value={values.adapter} onChange={(adapter) => set({ adapter })} />
      <TextField label="base branch" value={values.base} onChange={(base) => set({ base })}
        error={shown.base} placeholder="HEAD" />
      {shell ? (
        <TextField wide label="command" value={values.command} onChange={(command) => set({ command })}
          error={shown.command} placeholder="npm test --watch" />
      ) : (
        <TextField wide label="prompt" value={values.prompt} onChange={(prompt) => set({ prompt })}
          placeholder="optional" />
      )}
      <div className="th-new-agent-actions">
        <Button type="submit" variant="primary" busy={busy} busyLabel="starting…">
          start
        </Button>
        {error && (
          <span className="th-field-error" role="alert">
            error: {error}
          </span>
        )}
      </div>
    </form>
  );
}
