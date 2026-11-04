import { Fragment } from "react";
import type { Task } from "../../../api/types";
import { StatusGlyph } from "../../../ui/StatusGlyph";
import { loopCounts } from "./loopStages";

export function LoopStrip({ tasks }: { readonly tasks: readonly Task[] }) {
  const loop = loopCounts(tasks);
  const last = loop.stages.length - 1;
  return (
    <div className="th-loop" aria-label="Tasks per stage">
      {loop.stages.map(({ stage, count }, index) => (
        <Fragment key={stage}>
          <span className="th-loop-stage" data-active={count > 0 ? "true" : undefined}>
            {stage} <b>{count}</b>
          </span>
          {index < last && (
            <span className="th-loop-arrow" aria-hidden="true">
              ›
            </span>
          )}
        </Fragment>
      ))}
      {loop.needsYou > 0 && (
        <span className="th-loop-stage th-loop-stage--you">
          <StatusGlyph state="waiting" />
          needs you <b>{loop.needsYou}</b>
        </span>
      )}
    </div>
  );
}
