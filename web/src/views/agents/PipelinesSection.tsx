import { useState } from "react";
import type { Pipeline } from "../../api/types";
import { usePipelines } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { EmptyState } from "../../ui/EmptyState";
import { Loading } from "../../ui/Loading";
import { PipelineEditor } from "./PipelineEditor";

type Selection = { readonly kind: "none" } | { readonly kind: "new" } | { readonly kind: "open"; readonly name: string };

function stepsText(count: number): string {
  return count === 1 ? "1 step" : `${count} steps`;
}

function PipelineRow({ pipeline, selected, onSelect }: {
  readonly pipeline: Pipeline;
  readonly selected: boolean;
  readonly onSelect: (name: string) => void;
}) {
  return (
    <button type="button" className="th-agent-row" aria-pressed={selected} onClick={() => onSelect(pipeline.name)}>
      <span className="th-agent-row-body">
        <span className="th-agent-row-head">
          <b>{pipeline.name}</b>
          {pipeline.default ? <span className="th-tag">default</span> : null}
          {pipeline.origin === "plugin" ? (
            <span className="th-tag" title="this pipeline comes from a plugin and is read only">
              {`plugin ${pipeline.plugin ?? ""}`.trim()}
            </span>
          ) : null}
        </span>
        <span className="th-agent-row-desc">{pipeline.description}</span>
        <span className="th-cap th-agent-row-meta">
          <span>{stepsText(pipeline.steps.length)}</span>
          {pipeline.repos.length > 0 ? <span>{pipeline.repos.join(", ")}</span> : null}
        </span>
      </span>
    </button>
  );
}

export function PipelinesSection() {
  const { data, loading, error, reload } = usePipelines();
  const [selection, setSelection] = useState<Selection>({ kind: "none" });
  const names = (data ?? []).map((pipeline) => pipeline.name);
  const selectedName = selection.kind === "open" ? selection.name : null;

  return (
    <>
      {error !== null ? <Banner level="error">{`pipelines not loaded: ${error}`}</Banner> : null}
      {data === null && loading ? (
        <div className="th-empty">
          <Loading verb="loading pipelines" />
        </div>
      ) : null}
      {data !== null ? (
        <div className="th-agents">
          <div className="th-agent-column">
            <div className="th-pipeline-bar">
              <Button size="sm" variant="primary" onClick={() => setSelection({ kind: "new" })}>
                new pipeline
              </Button>
            </div>
            {data.length === 0 ? (
              <EmptyState title="no pipelines yet">a pipeline lists the steps townhall runs for a task, and the agent type for each</EmptyState>
            ) : (
              <div className="th-agent-list">
                {data.map((pipeline) => (
                  <PipelineRow
                    key={pipeline.name}
                    pipeline={pipeline}
                    selected={pipeline.name === selectedName}
                    onSelect={(name) => setSelection({ kind: "open", name })}
                  />
                ))}
              </div>
            )}
          </div>
          {selection.kind === "none" ? (
            <EmptyState title="pick a pipeline to see its TOML" />
          ) : (
            <PipelineEditor
              key={selection.kind === "new" ? "new" : selection.name}
              name={selectedName}
              taken={names}
              onSaved={(name) => {
                setSelection({ kind: "open", name });
                reload();
              }}
              onDeleted={() => {
                setSelection({ kind: "none" });
                reload();
              }}
            />
          )}
        </div>
      ) : null}
    </>
  );
}
