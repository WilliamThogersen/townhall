import { useState, type CSSProperties, type KeyboardEvent, type RefObject } from "react";
import { focusRow, rowKeyOf } from "./rowFocus";
import { SpanRow } from "./SpanRow";
import { ticksOf, type TimeAxis } from "./timeAxis";
import type { TraceRow } from "./traceRows";
import { treeAction } from "./treeKeys";

export const AXIS_LABEL = "conversation · task";

export interface TraceTreeProps {
  readonly rows: readonly TraceRow[];
  readonly axis: TimeAxis;
  readonly treeRef: RefObject<HTMLDivElement | null>;
  readonly onOpen: (row: TraceRow) => void;
  readonly onToggle: (key: string) => void;
}

type VarStyle = CSSProperties & Record<`--${string}`, string | number>;

function AxisRow({ axis }: { readonly axis: TimeAxis }) {
  return (
    <div className="th-span th-span--axis" aria-hidden="true">
      <div className="th-span-label">{AXIS_LABEL}</div>
      <div className="th-span-track">
        {ticksOf(axis).map((tick) => (
          <span key={tick.at} className="th-tick" style={{ "--at": `${tick.at}%` } as VarStyle}>
            {tick.label}
          </span>
        ))}
      </div>
    </div>
  );
}

function nowFraction(axis: TimeAxis): number {
  return (axis.now - axis.start) / (axis.end - axis.start);
}

export function TraceTree({ rows, axis, treeRef, onOpen, onToggle }: TraceTreeProps) {
  const [active, setActive] = useState<string | null>(null);
  const tabStop = rows.some((row) => row.key === active) ? active : (rows[0]?.key ?? null);

  const onKeyDown = (e: KeyboardEvent<HTMLDivElement>) => {
    const key = rowKeyOf(e.target);
    const action = key === null ? null : treeAction(rows, key, e.key);
    if (action === null) return;
    e.preventDefault();
    if (action.kind === "focus") focusRow(treeRef.current, action.key);
    else if (action.kind === "toggle") onToggle(action.key);
    else onOpen(action.row);
  };

  return (
    <div className="th-trace-body" style={{ "--th-trace-ticks": axis.ticks } as VarStyle}>
      <div ref={treeRef} className="th-trace-rows" role="tree" aria-label="timeline" onKeyDown={onKeyDown}>
        <AxisRow axis={axis} />
        {rows.map((row) => (
          <SpanRow
            key={row.key}
            row={row}
            tabStop={row.key === tabStop}
            onOpen={onOpen}
            onToggle={onToggle}
            onActive={setActive}
          />
        ))}
        <div className="th-now" style={{ "--now": nowFraction(axis) } as VarStyle} aria-hidden="true" />
      </div>
    </div>
  );
}
