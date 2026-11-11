import { Fragment } from "react";
import type { Task } from "../../../api/types";
import { useAgentTypes, useRuns } from "../../../data";
import { StatusGlyph } from "../../../ui/StatusGlyph";
import { loopSummary, runSteps } from "../../../tasks/stage";

export interface LoopStripProps {
  readonly conversationId: string;
  readonly tasks: readonly Task[];
  readonly entryCount: number;
}

function watchKey(tasks: readonly Task[], entryCount: number): string {
  return `${entryCount}|${tasks.map((task) => `${task.name}:${task.state}`).join(",")}`;
}

function Arrow() {
  return (
    <span className="th-loop-arrow" aria-hidden="true">
      ›
    </span>
  );
}

function NeedsYou({ count }: { readonly count: number }) {
  if (count === 0) return null;
  return (
    <span className="th-loop-stage th-loop-stage--you">
      <StatusGlyph state="waiting" />
      needs you <b>{count}</b>
    </span>
  );
}

export function LoopStrip({ conversationId, tasks, entryCount }: LoopStripProps) {
  const types = useAgentTypes().data ?? [];
  const run = useRuns(conversationId, watchKey(tasks, entryCount)).runs[0];
  const loop = loopSummary(tasks, undefined, types);

  if (run !== undefined && run.steps.length > 0) {
    const steps = runSteps(run);
    const needsYou = loop.needsYou > 0 ? loop.needsYou : run.state === "needsYou" ? 1 : 0;
    return (
      <div className="th-loop" aria-label="Steps of the run">
        {steps.map((step, index) => (
          <Fragment key={step.id}>
            <span className="th-loop-stage" data-active={step.active ? "true" : undefined} title={step.title}>
              {step.id} <b>{step.mark}</b>
            </span>
            {index < steps.length - 1 && <Arrow />}
          </Fragment>
        ))}
        <NeedsYou count={needsYou} />
      </div>
    );
  }

  const last = loop.stages.length - 1;
  return (
    <div className="th-loop" aria-label="Tasks per stage">
      {loop.stages.map(({ stage, count }, index) => (
        <Fragment key={stage}>
          <span className="th-loop-stage" data-active={count > 0 ? "true" : undefined}>
            {stage} <b>{count}</b>
          </span>
          {index < last && <Arrow />}
        </Fragment>
      ))}
      <NeedsYou count={loop.needsYou} />
    </div>
  );
}
