import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import type { ConversationDetail } from "../../api/types";
import { Button } from "../../ui/Button";
import { Spinner } from "../../ui/Spinner";
import { SESSION_PANEL_ID } from "./session";
import { needsYouCount, unfinishedCount } from "./thread/taskView";
import { conversationStatus, endQuestion, type StatusWords } from "./thread/threadModel";

export interface ConversationHeaderProps {
  readonly detail: ConversationDetail;
  readonly repo: string;
  readonly railOpen: boolean;
  readonly onToggleRail: () => void;
  readonly onStop: () => Promise<void>;
  readonly onEnd: () => Promise<void>;
}

const STATUS_KEY: Readonly<Record<StatusWords, string>> = {
  thinking: "thinking",
  "your turn": "yourTurn",
  ended: "ended",
};

function StatusWord({ status }: { readonly status: StatusWords }) {
  return (
    <span className="th-convo-head-status" data-status={STATUS_KEY[status]}>
      {status === "thinking" ? <Spinner /> : null}
      {status}
    </span>
  );
}

function useBusyAction(action: () => Promise<void>) {
  const [busy, setBusy] = useState(false);
  const run = async () => {
    setBusy(true);
    try {
      await action();
    } finally {
      setBusy(false);
    }
  };
  return { busy, run };
}

function useEndConfirm(unfinished: number, onEnd: () => Promise<void>) {
  const [asking, setAsking] = useState(false);
  const ending = useBusyAction(async () => {
    await onEnd();
    setAsking(false);
  });
  const endButton = useRef<HTMLButtonElement>(null);
  const cancelButton = useRef<HTMLButtonElement>(null);
  const wasAsking = useRef(false);
  useEffect(() => {
    if (asking) cancelButton.current?.focus();
    else if (wasAsking.current) endButton.current?.focus();
    wasAsking.current = asking;
  }, [asking]);
  const ask = () => (unfinished > 0 ? setAsking(true) : void ending.run());
  const cancel = () => setAsking(false);
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== "Escape" || ending.busy) return;
    e.stopPropagation();
    cancel();
  };
  return { asking, busy: ending.busy, confirm: ending.run, ask, cancel, onKeyDown, endButton, cancelButton };
}

function RailToggle({ open, needsYou, onToggle }: { readonly open: boolean; readonly needsYou: number; readonly onToggle: () => void }) {
  return (
    <Button
      variant="ghost"
      size="sm"
      className="th-rail-toggle"
      aria-expanded={open}
      aria-controls={SESSION_PANEL_ID}
      title="Show or hide the session panel (s)"
      onClick={onToggle}
    >
      {needsYou > 0 ? (
        <>
          <span className="th-status" data-state="waiting" aria-hidden="true" />
          {needsYou} ·{" "}
        </>
      ) : null}
      session
    </Button>
  );
}

export function ConversationHeader({ detail, repo, railOpen, onToggleRail, onStop, onEnd }: ConversationHeaderProps) {
  const { conversation, tasks } = detail;
  const status = conversationStatus(conversation);
  const unfinished = unfinishedCount(tasks);
  const end = useEndConfirm(unfinished, onEnd);
  const stop = useBusyAction(onStop);
  return (
    <header className="th-convo-head" data-confirm={end.asking || undefined} onKeyDown={end.asking ? end.onKeyDown : undefined}>
      <h2 className="th-convo-head-title" title={conversation.title}>
        {conversation.title}
      </h2>
      {end.asking ? null : (
        <>
          <span className="th-convo-head-repo">{repo}</span>
          <StatusWord status={status} />
        </>
      )}
      {end.asking ? (
        <div className="th-convo-head-actions">
          <span className="th-confirm">
            <span>{endQuestion(conversation.title, unfinished)}</span>
          </span>
          <Button ref={end.cancelButton} variant="ghost" size="sm" onClick={end.cancel} disabled={end.busy}>
            cancel
          </Button>
          <Button size="sm" onClick={() => void end.confirm()} busy={end.busy} busyLabel="ending…">
            end
          </Button>
        </div>
      ) : (
        <div className="th-convo-head-actions">
          {status === "thinking" ? (
            <Button variant="danger" size="sm" onClick={() => void stop.run()} busy={stop.busy} busyLabel="stopping…">
              stop
            </Button>
          ) : null}
          <RailToggle open={railOpen} needsYou={needsYouCount(tasks)} onToggle={onToggleRail} />
          {conversation.ended ? null : (
            <Button ref={end.endButton} variant="ghost" size="sm" onClick={end.ask} busy={end.busy} busyLabel="ending…">
              end
            </Button>
          )}
        </div>
      )}
    </header>
  );
}
