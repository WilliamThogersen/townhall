import { useState, type FormEvent } from "react";
import { sendText } from "../api/client";
import { toMessage } from "../api/errors";

export function SendBox({ name }: { name: string }) {
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: FormEvent) => {
    e.preventDefault();
    if (text.trim() === "") return;
    setBusy(true);
    setError(null);
    try {
      await sendText(name, text);
      setText("");
    } catch (err) {
      setError(toMessage(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form className="sendbox" onSubmit={(e) => void submit(e)}>
      <input
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Send input to the agent"
        aria-label={`Input for ${name}`}
        disabled={busy}
      />
      <button type="submit" disabled={busy || text.trim() === ""}>
        {busy ? "Sending..." : "Send"}
      </button>
      {error && <p className="error" role="alert">{error}</p>}
    </form>
  );
}
