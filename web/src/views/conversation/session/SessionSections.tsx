import type { CSSProperties } from "react";
import { Thumbs } from "../../../attachments";
import type { ContextUsage, ConversationDetail } from "../../../api/types";
import { contextView } from "./contextUsage";
import { repoLine } from "./repoLine";

function FaintLine({ text }: { readonly text: string }) {
  return (
    <div className="th-session-line">
      <span>{text}</span>
    </div>
  );
}

export function RepoSection({ detail }: { readonly detail: ConversationDetail }) {
  const line = repoLine(detail);
  if (line === null) return <FaintLine text="no repo yet" />;
  return (
    <div className="th-session-line" data-line="repo">
      <span>{line.label}</span>
      <span>{line.reason}</span>
    </div>
  );
}

export function FilesSection({ detail }: { readonly detail: ConversationDetail }) {
  const { conversation, attachments } = detail;
  if (attachments.length === 0) return <FaintLine text="no files yet" />;
  return (
    <div className="th-session-files">
      <Thumbs
        conversationId={conversation.id}
        attachmentIds={attachments.map((attachment) => attachment.id)}
        attachments={attachments}
        collapse={false}
      />
    </div>
  );
}

export function ContextSection({ usage }: { readonly usage: ContextUsage | null }) {
  const view = contextView(usage);
  if (view === null) return <FaintLine text="no context yet" />;
  const bar = { "--used": `${view.percent}%` } as CSSProperties;
  return (
    <div className="th-session-line" data-line="ctx">
      <span className="th-ctx" data-level={view.high ? "high" : undefined}>
        <span className="th-ctx-bar" style={bar} />
        {view.percent}%
      </span>
      <span>{view.words}</span>
    </div>
  );
}

export function TasksEmpty() {
  return <FaintLine text="no tasks yet" />;
}
