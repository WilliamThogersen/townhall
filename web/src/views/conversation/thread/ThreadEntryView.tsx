import type { ThreadEntry } from "../../../api/types";
import { NeedsYouBlock, RepoAskBlock } from "./AskBlocks";
import { AgentEventLine, ErrorEvent, RepoEvent } from "./EventEntries";
import { Markdown } from "./markdown/Markdown";
import { TaskBlock } from "./TaskBlock";
import { Message, PlainText } from "./ThreadLines";

export interface ThreadEntryViewProps {
  readonly entry: ThreadEntry;
  readonly open: boolean;
}

export function ThreadEntryView({ entry, open }: ThreadEntryViewProps) {
  switch (entry.kind) {
    case "you":
      return (
        <Message you at={entry.at}>
          <div className="th-msg-body">
            <PlainText text={entry.text} />
          </div>
        </Message>
      );
    case "townhall":
      return (
        <Message at={entry.at}>
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
      return <NeedsYouBlock entry={entry} open={open} />;
    case "agent":
      return <AgentEventLine entry={entry} />;
    case "error":
      return <ErrorEvent entry={entry} />;
  }
}
