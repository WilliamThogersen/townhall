import { Fragment, type ReactNode } from "react";
import type { TaskState } from "../../../api/types";
import { Spinner } from "../../../ui/Spinner";
import { taskGlyph } from "./taskView";
import { clockTime } from "./threadModel";

export function TaskGlyph({ state }: { readonly state: TaskState }) {
  const glyph = taskGlyph(state);
  return (
    <span className="th-status" data-state={glyph} aria-hidden="true">
      {glyph === "working" ? <Spinner /> : null}
    </span>
  );
}

export function EntryTime({ at }: { readonly at: string }) {
  return (
    <time className="th-msg-time" dateTime={at}>
      {clockTime(at)}
    </time>
  );
}

export interface EventLineProps {
  readonly sign: ReactNode;
  readonly kind: string;
  readonly event?: string;
  readonly at?: string;
  readonly children: ReactNode;
}

export function EventLine({ sign, kind, event, at, children }: EventLineProps) {
  return (
    <div className="th-event" data-kind={kind} data-event={event}>
      <span className="th-msg-sign" aria-hidden="true">
        {sign}
      </span>
      <span>{children}</span>
      {at === undefined ? <span className="th-msg-time" /> : <EntryTime at={at} />}
    </div>
  );
}

export interface MessageProps {
  readonly you?: boolean;
  readonly at: string;
  readonly children: ReactNode;
}

export function Message({ you = false, at, children }: MessageProps) {
  return (
    <div className={you ? "th-msg th-msg--you" : "th-msg"}>
      <span className="th-msg-sign" aria-hidden="true">
        {you ? "›" : ""}
      </span>
      {children}
      <EntryTime at={at} />
    </div>
  );
}

export function PlainText({ text }: { readonly text: string }) {
  const paragraphs = text.split(/\n\s*\n/).filter((part) => part.trim() !== "");
  return (
    <>
      {paragraphs.map((paragraph, i) => (
        <p key={i}>
          {paragraph.split("\n").map((line, j) => (
            <Fragment key={j}>
              {j > 0 ? <br /> : null}
              {line}
            </Fragment>
          ))}
        </p>
      ))}
    </>
  );
}
