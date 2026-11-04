import { Fragment } from "react";
import type { LoopSummary } from "./stage";

export function LoopStrip({ loop }: { readonly loop: LoopSummary }) {
  const last = loop.stages.length - 1;
  return (
    <div className="th-loop" aria-label="Tasks per stage">
      {loop.stages.map(({ stage, count, active }, index) => (
        <Fragment key={stage}>
          <span className="th-loop-stage" data-active={active ? "true" : undefined}>
            {stage} <b>{count}</b>
          </span>
          {index < last ? (
            <span className="th-loop-arrow" aria-hidden="true">
              ›
            </span>
          ) : null}
        </Fragment>
      ))}
      {loop.needsYou > 0 ? (
        <span className="th-loop-stage th-loop-stage--you">
          <span className="th-status" data-state="waiting" aria-hidden="true" />
          needs you <b>{loop.needsYou}</b>
        </span>
      ) : null}
    </div>
  );
}
