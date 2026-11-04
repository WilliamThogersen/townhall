import { useState, type FormEvent } from "react";
import { sendText } from "../api/client";
import { toMessage } from "../api/errors";
import { Button } from "../ui/Button";
import { Kbd } from "../ui/Kbd";

export function PanePrompt({ name }: { name: string }) {
  const [text, setText] = useState("");
  const [sending, setSending] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const empty = text.trim() === "";

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (empty || sending) return;
    setSending(true);
    setError(null);
    try {
      await sendText(name, text);
      setText("");
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setSending(false);
    }
  };

  return (
    <form className="th-prompt" onSubmit={(e) => void submit(e)}>
      <span className="th-prompt-sign" aria-hidden="true">
        ›
      </span>
      <input
        className="th-prompt-input"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder={`type into ${name}`}
        aria-label={`Input for ${name}`}
        autoComplete="off"
        spellCheck={false}
      />
      {error && (
        <span className="th-field-error" role="alert">
          error: {error}
        </span>
      )}
      <Button type="submit" size="sm" disabled={empty} busy={sending} busyLabel="sending…">
        send <Kbd>⏎</Kbd>
      </Button>
    </form>
  );
}
