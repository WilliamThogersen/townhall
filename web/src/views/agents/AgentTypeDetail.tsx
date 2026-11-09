import { useEffect, useState, type ReactNode } from "react";
import { useAgentType } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Loading";
import { compactNumber } from "../home/format";
import { AgentAvatar } from "./AgentAvatar";
import type { Usage } from "./agentTypes";
import { AvatarEditor } from "./AvatarEditor";
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
        <AgentAvatar name={data.name} scale={4} />
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
        </dl>
      </Section>
      {usage === null ? null : (
        <Section title="usage">
          <dl className="th-agent-fields">
            <Field label="runs">{String(usage.runs)}</Field>
            <Field label="tokens">{compactNumber(usage.tokens)}</Field>
          </dl>
        </Section>
      )}
      <Section title="avatar">
        <AvatarEditor name={data.name} />
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
