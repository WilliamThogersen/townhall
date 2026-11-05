import type { CSSProperties, MouseEvent } from "react";
import { Spinner } from "../../ui/Spinner";
import type { Bar, GlyphState, Tag } from "./traceBars";
import { rowLabel, type TraceRow } from "./traceRows";

export interface SpanRowProps {
  readonly row: TraceRow;
  readonly tabStop: boolean;
  readonly onOpen: (row: TraceRow) => void;
  readonly onToggle: (key: string) => void;
  readonly onActive: (key: string) => void;
}

type SpanStyle = CSSProperties & Record<`--${string}`, string | number>;

export function TraceGlyph({ state }: { readonly state: GlyphState }) {
  return (
    <span className="th-status" data-state={state} aria-hidden="true">
      {state === "working" ? <Spinner /> : null}
    </span>
  );
}

function tagClass(tag: Tag): string {
  return tag.tone ? `th-tag th-tag--${tag.tone}` : "th-tag";
}

function BarMarks({ bar }: { readonly bar: Bar }) {
  const style: SpanStyle = { "--start": `${bar.start}%`, "--len": `${bar.len}%` };
  return (
    <>
      <span className="th-bar" data-state={bar.state ?? undefined} style={style} />
      {bar.label ? (
        <span className="th-bar-label" style={style}>
          {bar.label}
        </span>
      ) : null}
    </>
  );
}

function Toggle({ row, onToggle }: Pick<SpanRowProps, "row" | "onToggle">) {
  if (row.expanded === null) return null;
  const flip = (e: MouseEvent) => {
    e.stopPropagation();
    onToggle(row.key);
  };
  return (
    <span className="th-span-toggle" aria-hidden="true" onClick={flip}>
      {row.expanded ? "▾" : "▸"}
    </span>
  );
}

export function SpanRow({ row, tabStop, onOpen, onToggle, onActive }: SpanRowProps) {
  const style: SpanStyle = { "--depth": row.depth };
  return (
    <div
      className="th-span"
      data-depth={row.depth}
      data-kind={row.kind}
      data-row={row.key}
      style={style}
      role="treeitem"
      aria-level={row.depth + 1}
      aria-expanded={row.expanded ?? undefined}
      aria-label={rowLabel(row)}
      tabIndex={tabStop ? 0 : -1}
      onFocus={() => onActive(row.key)}
      onClick={() => (row.kind === "task" ? onToggle(row.key) : onOpen(row))}
    >
      <div className="th-span-label">
        <Toggle row={row} onToggle={onToggle} />
        {row.glyph ? <TraceGlyph state={row.glyph} /> : null}
        <span className="th-span-name">{row.name}</span>
        {row.meta ? <span className="th-span-meta">{row.meta}</span> : null}
        {row.tag ? <span className={tagClass(row.tag)}>{row.tag.text}</span> : null}
      </div>
      <div className="th-span-track">{row.bar ? <BarMarks bar={row.bar} /> : null}</div>
    </div>
  );
}
