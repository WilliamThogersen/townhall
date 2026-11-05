import { useState, type FormEvent, type ReactNode } from "react";
import type { ThreadEntry } from "../../../api/types";
import { Button } from "../../../ui/Button";
import { useThread } from "./threadContext";
import { Message } from "./ThreadLines";
import { askChoices } from "./threadModel";
import { choiceKeys, takesKeys } from "./trustDialog";

type NeedsYouEntry = Extract<ThreadEntry, { kind: "needsYou" }>;
type RepoAskEntry = Extract<ThreadEntry, { kind: "repoAsk" }>;

function useBusy() {
  const [busy, setBusy] = useState(false);
  const run = async (action: () => Promise<boolean>): Promise<boolean> => {
    setBusy(true);
    try {
      return await action();
    } finally {
      setBusy(false);
    }
  };
  return { busy, run };
}

interface AskProps {
  readonly who: string;
  readonly open: boolean;
  readonly question: string;
  readonly children?: ReactNode;
}

function Ask({ who, open, question, children }: AskProps) {
  return (
    <div className="th-ask" role="group" aria-label={`${who} needs you`} data-answered={open ? undefined : "true"}>
      <div className="th-ask-head">
        <span className="th-status" data-state="waiting" aria-hidden="true" />
        {open ? `${who} needs you` : `${who} asked`}
      </div>
      <p className="th-ask-quote">{question}</p>
      {children}
    </div>
  );
}

interface AnswerFormProps {
  readonly agent: string;
  readonly busy: boolean;
  readonly onAnswer: (text: string) => Promise<boolean>;
}

function AnswerForm({ agent, busy, onAnswer }: AnswerFormProps) {
  const [text, setText] = useState("");
  const submit = async (e: FormEvent) => {
    e.preventDefault();
    const answer = text.trim();
    if (answer === "" || busy) return;
    if (await onAnswer(answer)) setText("");
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
        placeholder={`answer ${agent}`}
        aria-label={`Answer for ${agent}`}
        disabled={busy}
      />
      <Button size="sm" type="submit" disabled={busy || text.trim() === ""}>
        send
      </Button>
    </form>
  );
}

export function NeedsYouBlock({ entry, open }: { readonly entry: NeedsYouEntry; readonly open: boolean }) {
  const { tasks, onOpenTask, answerAgent, answerKeys } = useThread();
  const { busy, run } = useBusy();
  const isTask = tasks.some((task) => task.name === entry.agent);
  const answer = (text: string) => run(() => answerAgent(entry.agent, text));
  const pick = (choice: string) => {
    const keys = choiceKeys(entry.question, choice);
    return keys === null ? answer(choice) : run(() => answerKeys(entry.agent, keys));
  };
  const choices = open ? entry.choices : [];
  return (
    <Message at={entry.at}>
      <Ask who={entry.agent} open={open} question={entry.question}>
        {choices.length > 0 || isTask ? (
          <div className="th-ask-actions">
            {choices.map((choice) => (
              <Button key={choice} size="sm" disabled={busy} onClick={() => void pick(choice)}>
                send {choice}
              </Button>
            ))}
            {isTask ? (
              <Button variant="ghost" size="sm" onClick={() => onOpenTask(entry.agent)}>
                open terminal
              </Button>
            ) : null}
          </div>
        ) : null}
        {open && !takesKeys(entry.question) ? <AnswerForm agent={entry.agent} busy={busy} onAnswer={answer} /> : null}
      </Ask>
    </Message>
  );
}

export function RepoAskBlock({ entry, open }: { readonly entry: RepoAskEntry; readonly open: boolean }) {
  const { thinking, sendMessage } = useThread();
  const { busy, run } = useBusy();
  const { candidates, all } = askChoices(entry.candidates, entry.offerAll);
  const pick = (text: string) => void run(() => sendMessage(text));
  const disabled = busy || thinking;
  return (
    <Message at={entry.at}>
      <Ask who="townhall" open={open} question={entry.question}>
        {open ? (
          <div className="th-ask-actions">
            {candidates.map((name) => (
              <Button key={name} size="sm" disabled={disabled} onClick={() => pick(name)}>
                {name}
              </Button>
            ))}
            {all === null ? null : (
              <Button variant="ghost" size="sm" disabled={disabled} onClick={() => pick(all)}>
                {all}
              </Button>
            )}
          </div>
        ) : null}
      </Ask>
    </Message>
  );
}
