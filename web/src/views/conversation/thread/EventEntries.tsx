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
};

export function RepoEvent({ entry }: { readonly entry: Entry<"repo"> }) {
  return (
    <EventLine sign="~" kind="repo" at={entry.at}>
      repo <b>{entry.repo}</b> · {entry.baseBranch}, {repoReason(entry)}
    </EventLine>
  );
}

function agentText(entry: Entry<"agent">) {
  const text = entry.text.trim();
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
    <EventLine sign={AGENT_SIGN[entry.event]} kind="agent" event={entry.event} at={entry.at}>
      {agentText(entry)}
    </EventLine>
  );
}

export function ErrorEvent({ entry }: { readonly entry: Entry<"error"> }) {
  return (
    <EventLine sign="×" kind="error" at={entry.at}>
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
