import type { ReactNode } from "react";
import type { ConnectionStatus } from "../../data";
import { Link } from "../../router";
import { DRAG_REGION } from "../../ui/dragRegion";

const LIVE_WORDS: Readonly<Record<ConnectionStatus, string>> = {
  open: "● live",
  connecting: "connecting…",
  closed: "reconnecting…",
};

export interface ControlHeadProps {
  readonly caption: string | null;
  readonly connection: ConnectionStatus;
  readonly allConversationsHref: string | null;
  readonly children?: ReactNode;
}

export function ControlHead({ caption, connection, allConversationsHref, children }: ControlHeadProps) {
  return (
    <header className="th-page-head" {...DRAG_REGION}>
      <h1>control center</h1>
      {caption ? <span className="th-control-cap">{caption}</span> : null}
      <div className="th-page-head-actions">
        {allConversationsHref ? (
          <Link className="th-btn th-btn--ghost th-btn--sm" to={allConversationsHref} replace>
            all conversations
          </Link>
        ) : null}
        <span className="th-stream" data-status={connection === "open" ? "open" : undefined}>
          {LIVE_WORDS[connection]}
        </span>
        {children}
      </div>
    </header>
  );
}
