import { useEffect, useRef, useState, type CSSProperties, type RefObject } from "react";
import type { ClaudeUsage, UsageWindow } from "../api/types";
import { useUsage } from "../data";
import { useNow } from "../sidebar/useNow";
import {
  displayWindow,
  footLabel,
  isStale,
  meterLabel,
  staleReason,
  usageLevel,
  windowView,
  type WindowView,
} from "./usageModel";

function useOutsideMouseDown(ref: RefObject<HTMLElement | null>, active: boolean, onOutside: () => void) {
  const latest = useRef(onOutside);
  useEffect(() => {
    latest.current = onOutside;
  });
  useEffect(() => {
    if (!active) return;
    const onMouseDown = (event: MouseEvent) => {
      if (event.target instanceof Node && ref.current?.contains(event.target)) return;
      latest.current();
    };
    document.addEventListener("mousedown", onMouseDown, true);
    return () => document.removeEventListener("mousedown", onMouseDown, true);
  }, [ref, active]);
}

function useEscape(active: boolean, onEscape: () => void) {
  const latest = useRef(onEscape);
  useEffect(() => {
    latest.current = onEscape;
  });
  useEffect(() => {
    if (!active) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape") return;
      event.preventDefault();
      latest.current();
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [active]);
}

function Meter({ name, window, now }: { readonly name: string; readonly window: UsageWindow; readonly now: number }) {
  const shown = displayWindow(window, now);
  const used = Math.min(100, Math.max(0, shown.usedPercent));
  const bar = { "--used": `${used}%` } as CSSProperties;
  return (
    <span className="th-usage-meter" data-level={usageLevel(shown.usedPercent)} title={meterLabel(name, window, now)}>
      <span className="th-usage-track">
        <span className="th-usage-fill" style={bar} />
      </span>
      <span className="th-usage-num">
        {Math.round(shown.usedPercent)}% {name}
      </span>
    </span>
  );
}

function Row({
  view,
  window,
  now,
}: {
  readonly view: WindowView;
  readonly window: UsageWindow;
  readonly now: number;
}) {
  const shown = displayWindow(window, now);
  const used = Math.min(100, Math.max(0, shown.usedPercent));
  const bar = { "--used": `${used}%` } as CSSProperties;
  return (
    <div className="th-usage-row">
      <div className="th-usage-head">
        <span className="th-usage-name">{view.name}</span>
        <span className="th-usage-used">{view.used}</span>
      </div>
      <div
        className="th-usage-track th-usage-bar"
        role="progressbar"
        aria-label={view.name}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(used)}
        data-level={usageLevel(shown.usedPercent)}
      >
        <span className="th-usage-fill" style={bar} />
      </div>
      <div className="th-usage-details">
        <span className="th-usage-detail">{view.reset}</span>
        {view.runOut !== null && (
          <span className="th-usage-detail" data-warn={view.runsOut ? "true" : undefined}>
            {view.runOut}
          </span>
        )}
      </div>
    </div>
  );
}

function Popover({ usage, now }: { readonly usage: ClaudeUsage; readonly now: number }) {
  const why = isStale(usage) ? staleReason(usage.error) : "";
  return (
    <div className="th-usage-pop" role="dialog" aria-label="Claude plan usage">
      {usage.session && (
        <Row view={windowView("5 hours", usage.session, usage.updatedAt, now)} window={usage.session} now={now} />
      )}
      {usage.weekly && (
        <Row view={windowView("Week", usage.weekly, usage.updatedAt, now)} window={usage.weekly} now={now} />
      )}
      <p className="th-usage-foot">
        {footLabel(usage)}
        {why && <span className="th-usage-why">{why}</span>}
      </p>
    </div>
  );
}

export function UsageMeter() {
  const { usage } = useUsage();
  const now = useNow();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  useOutsideMouseDown(root, open, () => setOpen(false));
  useEscape(open, () => {
    setOpen(false);
    trigger.current?.focus();
  });

  if (usage === null || (usage.session === null && usage.weekly === null)) return null;

  return (
    <div ref={root} className="th-usage" data-stale={isStale(usage) ? "true" : undefined}>
      <button
        ref={trigger}
        type="button"
        className="th-usage-btn"
        aria-label="Claude plan usage"
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((was) => !was)}
      >
        <svg className="th-usage-icon" viewBox="0 0 8 8" aria-hidden="true">
          <path d="M4 0.5V7.5M0.97 2.25L7.03 5.75M0.97 5.75L7.03 2.25" />
        </svg>
        {usage.session && <Meter name="5h" window={usage.session} now={now} />}
        {usage.weekly && <Meter name="wk" window={usage.weekly} now={now} />}
      </button>
      {open && <Popover usage={usage} now={now} />}
    </div>
  );
}
