import { useState, type FormEvent } from "react";
import { createAgent } from "../api/client";
import { toMessage } from "../api/errors";
import type { Adapter } from "../api/types";
import { EMPTY_FORM, validateForm, type FormValues } from "./validation";

export function CreateAgentForm({ onCreated }: { onCreated: (name: string) => void }) {
  const [values, setValues] = useState<FormValues>(EMPTY_FORM);
  const [touched, setTouched] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const { errors, input } = validateForm(values);
  const set = (patch: Partial<FormValues>) => setValues((v) => ({ ...v, ...patch }));
  const show = (k: keyof typeof errors) => (touched ? errors[k] : undefined);

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

  return (
    <form className="create" onSubmit={(e) => void submit(e)} noValidate>
      <label>
        Name
        <input value={values.name} onChange={(e) => set({ name: e.target.value })}
          aria-invalid={Boolean(show("name"))} placeholder="fix-login" />
        {show("name") && <span className="error" role="alert">{show("name")}</span>}
      </label>
      <label>
        Adapter
        <select value={values.adapter} onChange={(e) => set({ adapter: e.target.value as Adapter })}>
          <option value="claude">claude</option>
          <option value="shell">shell</option>
        </select>
      </label>
      {values.adapter === "claude" ? (
        <label>
          Prompt (optional)
          <textarea value={values.prompt} rows={2} onChange={(e) => set({ prompt: e.target.value })} />
        </label>
      ) : (
        <label>
          Command
          <input value={values.command} onChange={(e) => set({ command: e.target.value })}
            aria-invalid={Boolean(show("command"))} placeholder="npm test --watch" />
          {show("command") && <span className="error" role="alert">{show("command")}</span>}
        </label>
      )}
      <label>
        Base (optional)
        <input value={values.base} onChange={(e) => set({ base: e.target.value })}
          aria-invalid={Boolean(show("base"))} placeholder="HEAD" />
        {show("base") && <span className="error" role="alert">{show("base")}</span>}
      </label>
      <button type="submit" className="primary" disabled={busy}>
        {busy ? "Creating..." : "Create agent"}
      </button>
      {error && <p className="error" role="alert">{error}</p>}
    </form>
  );
}
