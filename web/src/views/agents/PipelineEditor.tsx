import { useState } from "react";
import { definitionErrors, deletePipeline, savePipeline } from "../../api/pipelines";
import type { DefinitionError } from "../../api/types";
import { usePipeline } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Loading";
import { NEW_PIPELINE, errorLine, problem, sourceName } from "./pipelineSource";

interface FormProps {
  readonly title: string;
  readonly initial: string;
  readonly existing: string | null;
  readonly taken: readonly string[];
  readonly plugin: string | null;
  readonly onSaved: (name: string) => void;
  readonly onDeleted: () => void;
}

function PipelineForm({ title, initial, existing, taken, plugin, onSaved, onDeleted }: FormProps) {
  const [text, setText] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const [problems, setProblems] = useState<readonly DefinitionError[]>([]);
  const [refused, setRefused] = useState<string | null>(null);
  const readOnly = plugin !== null;

  const edit = (next: string) => {
    setText(next);
    setProblems([]);
    setRefused(null);
  };

  const refuse = (error: unknown) => {
    setBusy(false);
    const found = definitionErrors(error);
    if (found.length > 0) setProblems(found);
    else setRefused(error instanceof Error ? error.message : String(error));
  };

  const save = () => {
    const target = existing ?? sourceName(text);
    if (target === null) {
      setProblems([problem("name", 'add name = "…" on its own line')]);
      return;
    }
    if (existing === null && taken.some((name) => name.toLowerCase() === target.toLowerCase())) {
      setProblems([problem("name", `a pipeline called "${target}" exists: open it to edit`)]);
      return;
    }
    setBusy(true);
    setProblems([]);
    setRefused(null);
    savePipeline(target, text).then((saved) => {
      setBusy(false);
      onSaved(saved.name);
    }, refuse);
  };

  const remove = () => {
    if (existing === null) return;
    setBusy(true);
    setRefused(null);
    deletePipeline(existing).then(() => {
      setBusy(false);
      onDeleted();
    }, refuse);
  };

  return (
    <section className="th-agent-detail th-pipeline-detail" aria-label={title}>
      <header className="th-agent-detail-head">
        <h2>{title}</h2>
        {readOnly ? (
          <span className="th-tag" title="this pipeline comes from a plugin and is read only">{`plugin ${plugin}`}</span>
        ) : null}
        {readOnly ? null : (
          <div className="th-page-head-actions">
            {confirming ? (
              <span className="th-pipeline-confirm" role="group" aria-label={`delete ${title}`}>
                <span className="th-confirm">{`delete ${title}? runs already started keep their copy`}</span>
                <Button size="sm" variant="ghost" disabled={busy} onClick={() => setConfirming(false)}>
                  cancel
                </Button>
                <Button size="sm" variant="danger-solid" busy={busy} busyLabel="deleting…" aria-label="confirm delete" onClick={remove}>
                  delete
                </Button>
              </span>
            ) : (
              <>
                {existing === null ? null : (
                  <Button size="sm" variant="ghost" disabled={busy} onClick={() => setConfirming(true)}>
                    delete
                  </Button>
                )}
                <Button size="sm" variant="primary" busy={busy} disabled={text === initial} onClick={save}>
                  save
                </Button>
              </>
            )}
          </div>
        )}
      </header>
      <textarea
        className="th-input th-pipeline-source"
        aria-label="pipeline toml"
        aria-invalid={problems.length > 0 ? true : undefined}
        spellCheck={false}
        readOnly={readOnly}
        rows={22}
        value={text}
        onChange={(event) => edit(event.target.value)}
      />
      {problems.length > 0 ? (
        <ul aria-label="pipeline errors" className="th-pipeline-errors">
          {problems.map((error) => {
            const line = errorLine(error);
            return <li key={line}>{line}</li>;
          })}
        </ul>
      ) : null}
      {refused === null ? null : <Banner level="error">{refused}</Banner>}
    </section>
  );
}

export interface PipelineEditorProps {
  readonly name: string | null;
  readonly taken: readonly string[];
  readonly onSaved: (name: string) => void;
  readonly onDeleted: () => void;
}

function Existing({ name, taken, onSaved, onDeleted }: PipelineEditorProps & { readonly name: string }) {
  const { data, error, loading, reload } = usePipeline(name);
  if (error !== null && data === null) return <Banner level="error">{error}</Banner>;
  if (data === null || loading) {
    return (
      <div className="th-empty">
        <Loading verb="loading pipeline" />
      </div>
    );
  }
  return (
    <PipelineForm
      key={`${data.updatedAt}:${data.source}`}
      title={data.name}
      initial={data.source}
      existing={data.name}
      taken={taken}
      plugin={data.origin === "plugin" ? (data.plugin ?? "plugin") : null}
      onSaved={(savedName) => {
        reload();
        onSaved(savedName);
      }}
      onDeleted={onDeleted}
    />
  );
}

export function PipelineEditor(props: PipelineEditorProps) {
  const { name, taken, onSaved, onDeleted } = props;
  if (name === null) {
    return (
      <PipelineForm
        title="new pipeline"
        initial={NEW_PIPELINE}
        existing={null}
        taken={taken}
        plugin={null}
        onSaved={onSaved}
        onDeleted={onDeleted}
      />
    );
  }
  return <Existing {...props} name={name} />;
}
