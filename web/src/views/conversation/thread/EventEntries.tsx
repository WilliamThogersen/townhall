import type { AgentEvent, EntryBody, ThreadEntry } from "../../../api/types";
import { Spinner } from "../../../ui/Spinner";
import { EventLine } from "./ThreadLines";
import { repoReason } from "./threadModel";

type Entry<K extends EntryBody["kind"]> = Extract<ThreadEntry, { kind: K }>;

const AGENT_SIGN: Readonly<Record<AgentEvent, string>> = {
  sent: "→",
  passed: "✓",
  failed: "×",
  ended: "○",
  allowed: "+",
};

export function RepoEvent({ entry }: { readonly entry: Entry<"repo"> }) {
  return (
    <EventLine sign="~" kind="repo" at={entry.at} entry={entry}>
      repo <b>{entry.repo}</b> · {entry.baseBranch}, {repoReason(entry)}
    </EventLine>
  );
}

const ALLOWED_TO = " to ";

function allowedText(entry: Entry<"agent">) {
  const cut = entry.text.lastIndexOf(ALLOWED_TO);
  const tool = cut === -1 ? entry.text : entry.text.slice(0, cut);
  const type = cut === -1 ? "" : entry.text.slice(cut + ALLOWED_TO.length);
  return (
    <>
      <b>{entry.agent}</b> allowed <code>{tool}</code>
      {type === "" ? null : (
        <>
          {" "}
          for agent type <b>{type}</b>
        </>
      )}
    </>
  );
}

function agentText(entry: Entry<"agent">) {
  const text = entry.text.trim();
  if (entry.event === "allowed") return allowedText(entry);
  if (entry.event === "sent") {
    return (
      <>
        sent to <b>{entry.agent}</b>
        {text === "" ? null : `: ${text}`}
      </>
    );
  }
  return (
    <>
      <b>{entry.agent}</b> {text === "" ? entry.event : text}
    </>
  );
}

export function AgentEventLine({ entry }: { readonly entry: Entry<"agent"> }) {
  return (
    <EventLine sign={AGENT_SIGN[entry.event]} kind="agent" event={entry.event} at={entry.at} entry={entry}>
      {agentText(entry)}
    </EventLine>
  );
}

export function TaskEvent({ entry }: { readonly entry: Entry<"event"> }) {
  return (
    <EventLine sign="◇" kind="event" at={entry.at} entry={entry}>
      {entry.text}
    </EventLine>
  );
}

export function PluginEvent({ entry }: { readonly entry: Entry<"plugin"> }) {
  return (
    <EventLine sign="◇" kind="plugin" at={entry.at} entry={entry}>
      <b>plugin {entry.plugin}</b>: {entry.text}
    </EventLine>
  );
}

export function HandoffEvent({ entry }: { readonly entry: Entry<"handoff"> }) {
  return (
    <EventLine sign="↩" kind="handoff" at={entry.at} entry={entry}>
      <b>{entry.task}</b>: {entry.from} handed back to <b>{entry.to}</b>, {entry.reason}
    </EventLine>
  );
}

export function ErrorEvent({ entry }: { readonly entry: Entry<"error"> }) {
  return (
    <EventLine sign="×" kind="error" at={entry.at} entry={entry}>
      error: {entry.message}
    </EventLine>
  );
}

export function ThinkingRow() {
  return (
    <div className="th-event" data-kind="thinking" role="status">
      <span className="th-msg-sign" aria-hidden="true">
        <Spinner />
      </span>
      <span>thinking</span>
      <span className="th-msg-time" />
    </div>
  );
}

export function EndedNote() {
  return (
    <EventLine sign="○" kind="ended">
      info: ended. a message continues it
    </EventLine>
  );
}
