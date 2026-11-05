import { useMemo, useState, type FormEvent, type ReactNode } from "react";
import type { ThreadEntry } from "../../../api/types";
import { Button } from "../../../ui/Button";
import { useThread } from "./threadContext";
import { answerText, askView, permissionTool, yesOption, type AskOption } from "./askView";
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
  /** Lines before the question, such as the command a permission dialog is about. */
  readonly context?: readonly string[];
  /** What the answer picked, once there was one. */
  readonly picked?: string | null;
  readonly children?: ReactNode;
}

function Ask({ who, open, question, context = [], picked = null, children }: AskProps) {
  const label = open ? `${who} needs you` : `${who} asked`;
  return (
    <div className="th-ask" role="group" aria-label={label} data-answered={open ? undefined : "true"}>
      <div className="th-ask-head">{label}</div>
      {context.map((line, i) => (
        <code key={i} className="th-ask-context" title={line}>
          {line}
        </code>
      ))}
      {question === "" ? null : <p className="th-ask-quote">{question}</p>}
      {picked === null ? null : (
        <p className="th-ask-answer">
          <span aria-hidden="true">{"\u2192"} </span>
          <span className="th-ask-picked">{picked}</span>
        </p>
      )}
      {children}
    </div>
  );
}

interface AskOptionsProps {
  readonly options: readonly AskOption[];
  readonly busy: boolean;
  readonly onPick: (number: string) => void;
}

function AskOptions({ options, busy, onPick }: AskOptionsProps) {
  return (
    <div className="th-ask-options">
      {options.map((option) => (
        <button
          key={option.number}
          type="button"
          className="th-ask-option"
          disabled={busy}
          aria-label={option.hint === null ? `${option.number} ${option.label}` : `${option.number} ${option.label}, ${option.hint}`}
          onClick={() => onPick(option.number)}
        >
          <span className="th-ask-key">{option.number}</span>
          <span className="th-ask-label">{option.label}</span>
          {option.hint === null ? null : <span className="th-ask-hint">{option.hint}</span>}
        </button>
      ))}
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

interface NeedsYouBlockProps {
  readonly entry: NeedsYouEntry;
  readonly open: boolean;
  /** The answer the agent got, when the thread has it. */
  readonly answer?: string;
}

interface AlwaysAllowProps {
  readonly agentType: string;
  readonly initial: string;
  readonly busy: boolean;
  readonly onAllow: (tool: string) => void;
}

function AlwaysAllow({ agentType, initial, busy, onAllow }: AlwaysAllowProps) {
  const [tool, setTool] = useState(initial);
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const pattern = tool.trim();
    if (pattern !== "" && !busy) onAllow(pattern);
  };
  return (
    <form className="th-ask-allow" onSubmit={submit}>
      <input
        className="th-prompt-input"
        value={tool}
        onChange={(e) => setTool(e.target.value)}
        aria-label={`Pattern to always allow for ${agentType}`}
        disabled={busy}
      />
      <Button size="sm" type="submit" disabled={busy || tool.trim() === ""}>
        always allow for {agentType}
      </Button>
      <p className="th-ask-allow-note">
        saved on {agentType} after this answer; applies from the next launch of {agentType}, this agent keeps the list it started with
      </p>
    </form>
  );
}

export function NeedsYouBlock({ entry, open, answer: sent }: NeedsYouBlockProps) {
  const { tasks, onOpenTask, answerAgent, answerKeys, allowTool } = useThread();
  const { busy, run } = useBusy();
  const view = useMemo(() => askView(entry.question, entry.choices), [entry.question, entry.choices]);
  const isTask = tasks.some((task) => task.name === entry.agent);
  const answer = (text: string) => run(() => answerAgent(entry.agent, text));
  const pick = (choice: string) => {
    const keys = choiceKeys(entry.question, choice);
    return keys === null ? answer(choice) : run(() => answerKeys(entry.agent, keys));
  };
  const hasOptions = view.options.length > 0;
  const task = tasks.find((t) => t.name === entry.agent);
  const suggestion = permissionTool(view.context);
  const yes = yesOption(view.options);
  const alwaysAllow = (tool: string) =>
    run(async () => {
      if (!(await allowTool(entry.agent, tool))) return false;
      return yes === null ? true : answerAgent(entry.agent, yes);
    });
  const choices = open && !hasOptions ? entry.choices : [];
  const picked = sent === undefined ? null : answerText(view, sent);
  return (
    <Message from="ask" at={entry.at} answered={!open}>
      <Ask who={entry.agent} open={open} question={view.prompt} context={view.context} picked={picked}>
        {open && hasOptions ? <AskOptions options={view.options} busy={busy} onPick={(n) => void pick(n)} /> : null}
        {open && task !== undefined && suggestion !== null && yes !== null ? (
          <AlwaysAllow
            key={suggestion}
            agentType={task.agent}
            initial={suggestion}
            busy={busy}
            onAllow={(tool) => void alwaysAllow(tool)}
          />
        ) : null}
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
        {open && !hasOptions && !takesKeys(entry.question) ? (
          <AnswerForm agent={entry.agent} busy={busy} onAnswer={answer} />
        ) : null}
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
    <Message from="ask" at={entry.at} answered={!open}>
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
