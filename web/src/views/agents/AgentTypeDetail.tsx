import type { ReactNode } from "react";
import { useAgentRuns, useAgentType } from "../../data";
import { Banner } from "../../ui/Banner";
import { Loading } from "../../ui/Loading";
import { compactNumber } from "../home/format";
import { AgentAvatar } from "./AgentAvatar";
import { AgentPrompt } from "./AgentPrompt";
import { CopyButton } from "./CopyButton";
import { AvatarEditor } from "./AvatarEditor";
import { StepBadge } from "./AgentTypeRow";
import { UsageGraph } from "./UsageGraph";

function Field({ label, children }: { readonly label: string; readonly children: ReactNode }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>{children}</dd>
    </>
  );
}

function Section({ title, children }: { readonly title: string; readonly children: ReactNode }) {
  return (
    <section className="th-agent-section" aria-label={title}>
      <h3 className="th-agent-section-title">{title}</h3>
      {children}
    </section>
  );
}

function Chips({ items }: { readonly items: readonly string[] }) {
  return (
    <span className="th-agent-chips">
      {items.map((item) => (
        <span key={item} className="th-tag">
          {item}
        </span>
      ))}
    </span>
  );
}

function HandBackTo({ allowed }: { readonly allowed: readonly string[] | null }) {
  if (allowed === null) return <>any type</>;
  return allowed.length > 0 ? <Chips items={allowed} /> : <>nobody</>;
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

export function AgentTypeDetail({ name, onSaved }: { readonly name: string; readonly onSaved: () => void }) {
  const { data, error, loading, reload } = useAgentType(name);
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
        <AgentAvatar name={data.name} spec={data.avatar} scale={4} />
        <h2>{data.name}</h2>
        <StepBadge name={data.name} />
        <div className="th-page-head-actions">
          <CopyButton text={data.source} />
        </div>
      </header>
      <Section title="setup">
        <dl className="th-agent-fields">
          <Field label="adapter">{data.adapter}</Field>
          <Field label="model">{data.model ?? "default"}</Field>
          <Field label="permission mode">{data.permissionMode ?? "default"}</Field>
          <Field label="tools">{data.tools.length > 0 ? <Chips items={data.tools} /> : "claude defaults"}</Field>
          <Field label="hands back to">
            <HandBackTo allowed={data.canHandBackTo} />
          </Field>
          <Field label="env">{envNames(data.env)}</Field>
        </dl>
      </Section>
      <Section title="prompt">
        <AgentPrompt prompt={data.prompt} />
      </Section>
      <Section title="usage">
        <UsageGraph name={data.name} />
      </Section>
      <RecentRuns name={data.name} />
      <Section title="avatar">
        <AvatarEditor
          key={`${data.updatedAt}:${data.source}`}
          name={data.name}
          source={data.source}
          saved={data.avatar}
          onSaved={() => {
            reload();
            onSaved();
          }}
        />
      </Section>
      <details className="th-agent-toml">
        <summary>toml</summary>
        <pre className="th-agent-source" tabIndex={0}>
          {data.source}
        </pre>
      </details>
    </section>
  );
}
