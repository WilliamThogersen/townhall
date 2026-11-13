import { useState, type ReactNode } from "react";
import type { AgentTypeMode } from "../../api/types";
import { useAgentRuns, useAgentType } from "../../data";
import { Link, PATHS } from "../../router";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Loading";
import { compactNumber } from "../home/format";
import { AgentAvatar } from "./AgentAvatar";
import { AgentPluginsPicker } from "./AgentPluginsPicker";
import { AgentPrompt } from "./AgentPrompt";
import { CopyButton } from "./CopyButton";
import { AvatarEditor } from "./AvatarEditor";
import { StepBadge } from "./AgentTypeRow";
import { isAdviserType, isConversationType } from "./agentTypes";
import { RUNS_SHOWN, runModel, shownRuns, showsModelColumn } from "./agentRuns";
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

function RecentRuns({ name, typeModel }: { readonly name: string; readonly typeModel: string | null }) {
  const { data, error } = useAgentRuns(name);
  const [all, setAll] = useState(false);
  const hidden = data === null ? 0 : data.length - RUNS_SHOWN;
  const withModel = data !== null && showsModelColumn(data, typeModel);
  return (
    <Section title="recent runs">
      {error !== null && data === null ? <Banner level="error">runs not loaded: {error}</Banner> : null}
      {data !== null && data.length === 0 ? <p className="th-cap">no runs yet</p> : null}
      {data !== null && data.length > 0 ? (
        <>
          <table className="th-agent-runs-table">
            <thead>
              <tr>
                <th>task</th>
                {withModel ? <th>model</th> : null}
                <th>tokens</th>
                <th>state</th>
                <th>started</th>
              </tr>
            </thead>
            <tbody>
              {shownRuns(data, all).map((run) => {
                const model = runModel(run, typeModel);
                return (
                  <tr key={`${run.kind}:${run.conversationId}:${run.task}:${run.createdAt}`}>
                    <td title={run.task}>
                      {run.kind === "turn" ? <Link to={PATHS.conversation(run.conversationId)}>{run.task}</Link> : run.task}
                    </td>
                    {withModel ? (
                      <td title={model?.inferred ? "the agent type's model; the run did not record one" : undefined} data-inferred={model?.inferred || undefined}>
                        {model?.name}
                      </td>
                    ) : null}
                    <td>{compactNumber(run.tokens)}</td>
                    <td>{run.state}</td>
                    <td>{run.createdAt.slice(0, 10)}</td>
                  </tr>
                );
              })}
            </tbody>
          </table>
          {hidden > 0 ? (
            <Button size="sm" variant="ghost" onClick={() => setAll((on) => !on)}>
              {all ? "show fewer" : `show ${hidden} more`}
            </Button>
          ) : null}
        </>
      ) : null}
    </Section>
  );
}

function modeText(mode: AgentTypeMode | null): string {
  if (mode === null) return "acceptEdits";
  return mode === "bypassPermissions" ? "bypassPermissions (skip all: never asks)" : mode;
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
        {data.origin === "plugin" ? (
          <span className="th-tag" title="this type comes from a plugin and is read only">
            {data.plugin === null || data.plugin === undefined ? "plugin" : `plugin ${data.plugin}`}
          </span>
        ) : null}
        <div className="th-page-head-actions">
          <CopyButton text={data.source} />
        </div>
      </header>
      {isConversationType(data.name) ? (
        <p className="th-cap">
          runs your conversations with townhall: its model, permission mode, tools and prompt apply to every turn. it can be edited but cannot be deleted, and tasks never default to it.
        </p>
      ) : null}
      {isAdviserType(data.name) ? (
        <p className="th-cap">
          other agents ask it with the ask_adviser tool instead of asking you. it only reads and answers: its model, effort, tools and prompt apply to every call. delete it and ask_adviser tells agents there is no adviser.
        </p>
      ) : null}
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
      <Section title="setup">
        <dl className="th-agent-fields">
          <Field label="adapter">{data.adapter}</Field>
          <Field label="model">{data.model ?? "default"}</Field>
          {data.effort ? <Field label="effort">{data.effort}</Field> : null}
          <Field label="permission mode">{modeText(data.permissionMode)}</Field>
          <Field label="tools">{data.tools.length > 0 ? <Chips items={data.tools} /> : "claude defaults"}</Field>
          <Field label="hands back to">
            <HandBackTo allowed={data.canHandBackTo} />
          </Field>
          <Field label="env">{envNames(data.env)}</Field>
        </dl>
      </Section>
      <Section title="plugins">
        <AgentPluginsPicker
          type={data}
          onSaved={() => {
            reload();
            onSaved();
          }}
        />
      </Section>
      <Section title="prompt">
        <AgentPrompt prompt={data.prompt} />
      </Section>
      <Section title="usage">
        <UsageGraph name={data.name} />
      </Section>
      <RecentRuns name={data.name} typeModel={data.model} />
      <details className="th-agent-toml">
        <summary>toml</summary>
        <pre className="th-agent-source" tabIndex={0}>
          {data.source}
        </pre>
      </details>
    </section>
  );
}
