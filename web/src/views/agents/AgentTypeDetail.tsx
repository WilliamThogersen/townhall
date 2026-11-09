import { useEffect, useState } from "react";
import { useAgentRuns, useAgentType } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Loading";
import { compactNumber } from "../home/format";
import type { Usage } from "./agentTypes";
import { StepBadge } from "./AgentTypeRow";

const COPIED_MS = 1500;

function CopyButton({ text }: { readonly text: string }) {
  const [copied, setCopied] = useState(false);
  useEffect(() => {
    if (!copied) return;
    const timer = setTimeout(() => setCopied(false), COPIED_MS);
    return () => clearTimeout(timer);
  }, [copied]);
  const copy = () => {
    navigator.clipboard.writeText(text).then(
      () => setCopied(true),
      () => setCopied(false),
    );
  };
  return (
    <Button size="sm" variant="ghost" onClick={copy} data-copied={copied || undefined}>
      {copied ? "copied" : "copy"}
    </Button>
  );
}

function Field({ label, value }: { readonly label: string; readonly value: string }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </>
  );
}

function handBackTo(allowed: readonly string[] | null): string {
  if (allowed === null) return "any type";
  return allowed.length > 0 ? allowed.join(", ") : "nobody";
}

function envNames(env: Readonly<Record<string, string>>): string {
  const names = Object.keys(env).sort();
  return names.length > 0 ? names.join(", ") : "none";
}

function RecentRuns({ name }: { readonly name: string }) {
  const { data, error } = useAgentRuns(name);
  return (
    <section className="th-agent-runs" aria-label="recent runs">
      <h3>recent runs</h3>
      {error !== null && data === null ? <Banner level="error">runs not loaded: {error}</Banner> : null}
      {data !== null && data.length === 0 ? <p>no runs yet</p> : null}
      {data !== null && data.length > 0 ? (
        <table>
          <thead>
            <tr>
              <th>task</th>
              <th>model</th>
              <th>tokens</th>
              <th>state</th>
              <th>started</th>
            </tr>
          </thead>
          <tbody>
            {data.map((run) => (
              <tr key={run.task}>
                <td>{run.task}</td>
                <td>{run.model ?? "unknown"}</td>
                <td>{compactNumber(run.tokens)}</td>
                <td>{run.state}</td>
                <td>{run.createdAt.slice(0, 10)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : null}
    </section>
  );
}

export function AgentTypeDetail({ name, usage }: { readonly name: string; readonly usage: Usage | null }) {
  const { data, error, loading } = useAgentType(name);
  if (error !== null && data === null) return <Banner level="error">{error}</Banner>;
  if (data === null || loading) {
    return (
      <div className="th-empty">
        <Loading verb="loading agent type" />
      </div>
    );
  }
  return (
    <section className="th-agent-detail" aria-label={data.name}>
      <header className="th-agent-detail-head">
        <h2>{data.name}</h2>
        <StepBadge name={data.name} />
        <div className="th-page-head-actions">
          <CopyButton text={data.source} />
        </div>
      </header>
      <dl className="th-agent-fields">
        <Field label="adapter" value={data.adapter} />
        <Field label="model" value={data.model ?? "default"} />
        <Field label="permission mode" value={data.permissionMode ?? "default"} />
        <Field label="tools" value={data.tools.length > 0 ? data.tools.join(", ") : "claude defaults"} />
        {usage === null ? null : (
          <>
            <Field label="runs" value={String(usage.runs)} />
            <Field label="tokens" value={compactNumber(usage.tokens)} />
          </>
        )}
        <Field label="hands back to" value={handBackTo(data.canHandBackTo)} />
        <Field label="env" value={envNames(data.env)} />
      </dl>
      <RecentRuns name={data.name} />
      <pre className="th-agent-source" tabIndex={0}>
        {data.source}
      </pre>
    </section>
  );
}
