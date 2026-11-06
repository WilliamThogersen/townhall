import type { ThreadEntry } from "../../../api/types";
import { stripAttachmentMarkers, Thumbs } from "../../../attachments";
import type { ReplyTarget } from "../../../composer/replies";
import { NeedsYouBlock, RepoAskBlock } from "./AskBlocks";
import { AgentEventLine, ErrorEvent, HandoffEvent, RepoEvent, TaskEvent } from "./EventEntries";
import { Markdown } from "./markdown/Markdown";
import { QuotedReply } from "./QuotedReply";
import { TaskBlock } from "./TaskBlock";
import { useThread } from "./threadContext";
import { Message, PlainText } from "./ThreadLines";

export interface ThreadEntryViewProps {
  readonly entry: ThreadEntry;
  readonly open: boolean;
  /** For an agent question: the answer the agent got, when there was one. */
  readonly answer?: string;
  /** For a reply: what the entry it answers said, null when that is gone or not quotable. */
  readonly quote?: ReplyTarget | null;
}

type YouEntry = Extract<ThreadEntry, { kind: "you" }>;

function YouMessage({ entry, quote }: { readonly entry: YouEntry; readonly quote: ReplyTarget | null }) {
  const { conversationId, attachments } = useThread();
  const names = attachments.filter((a) => entry.attachments.includes(a.id)).map((a) => a.name);
  return (
    <Message from="you" at={entry.at} entry={entry}>
      <div className="th-msg-body">
        {entry.replyTo === undefined ? null : <QuotedReply id={entry.replyTo} target={quote} />}
        <PlainText text={stripAttachmentMarkers(entry.text, names)} />
        <Thumbs conversationId={conversationId} attachmentIds={entry.attachments} attachments={attachments} />
      </div>
    </Message>
  );
}

export function ThreadEntryView({ entry, open, answer, quote = null }: ThreadEntryViewProps) {
  switch (entry.kind) {
    case "you":
      return <YouMessage entry={entry} quote={quote} />;
    case "townhall":
      return (
        <Message from="townhall" at={entry.at} entry={entry}>
          <div className="th-msg-body">
            <Markdown text={entry.text} />
          </div>
        </Message>
      );
    case "repo":
      return <RepoEvent entry={entry} />;
    case "repoAsk":
      return <RepoAskBlock entry={entry} open={open} />;
    case "tasksCreated":
      return <TaskBlock entry={entry} />;
    case "needsYou":
      return <NeedsYouBlock entry={entry} open={open} answer={answer} />;
    case "agent":
      return <AgentEventLine entry={entry} />;
    case "event":
      return <TaskEvent entry={entry} />;
    case "handoff":
      return <HandoffEvent entry={entry} />;
    case "error":
      return <ErrorEvent entry={entry} />;
  }
}
