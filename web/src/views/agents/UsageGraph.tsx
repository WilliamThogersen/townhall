import { useState } from "react";
import type { AgentUsageSeries, UsageRange, UsageShare } from "../../api/types";
import { useAgentUsageSeries } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Loading";
import { barPercents, formatSpan, runsLabel, tokensLabel } from "./usageSeries";

const RANGES: readonly UsageRange[] = ["24h", "7d", "30d"];
const DEFAULT_RANGE: UsageRange = "7d";

interface Pick {
  readonly hover: number | null;
  readonly focus: number | null;
  readonly pinned: number | null;
}

const NO_PICK: Pick = { hover: null, focus: null, pinned: null };

interface BarsProps {
  readonly label: string;
  readonly unit: (n: number) => string;
  readonly series: AgentUsageSeries;
  readonly values: readonly number[];
  readonly pick: Pick;
  readonly onPick: (change: Partial<Pick> | ((prev: Pick) => Partial<Pick>)) => void;
}

function Bars({ label, unit, series, values, pick, onPick }: BarsProps) {
  const heights = barPercents(values);
  return (
    <div role="group" aria-label={`${label} over time`} className="th-usage-bars" onMouseLeave={() => onPick({ hover: null })}>
      {series.buckets.map((bucket, at) => {
        const span = formatSpan(bucket.start, series.bucketMs);
        return (
          <button
            key={bucket.start}
            type="button"
            className="th-usage-bar"
            aria-label={`${span}: ${unit(values[at] ?? 0)}`}
            aria-pressed={pick.pinned === at}
            data-active={pick.hover === at || pick.focus === at || pick.pinned === at || undefined}
            onMouseEnter={() => onPick({ hover: at })}
            onFocus={() => onPick({ focus: at })}
            onBlur={() => onPick({ focus: null })}
            onClick={() => onPick((prev) => ({ pinned: prev.pinned === at ? null : at }))}
          >
            <span className="th-usage-bar-fill" style={{ height: `${heights[at] ?? 0}%` }} />
          </button>
        );
      })}
    </div>
  );
}

function Shares({ title, shares, peak }: { readonly title: string; readonly shares: readonly UsageShare[]; readonly peak: number }) {
  return (
    <div className="th-usage-shares">
      <h4 className="th-cap">{title}</h4>
      <ul aria-label={title} className="th-usage-share-list">
        {shares.map((share) => (
          <li key={share.key} className="th-usage-share">
            <span className="th-usage-share-key" title={share.key}>
              {share.key}
            </span>
            <span className="th-usage-share-bar" style={{ width: `${peak === 0 ? 0 : (share.tokens / peak) * 100}%` }} />
            <span className="th-usage-share-value">{`${tokensLabel(share.tokens)} · ${runsLabel(share.runs)}`}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Readout({ series, at }: { readonly series: AgentUsageSeries; readonly at: number | null }) {
  const bucket = at === null ? undefined : series.buckets[at];
  const text =
    bucket === undefined
      ? `last ${series.range}: ${runsLabel(series.runs)} · ${tokensLabel(series.tokens)}`
      : `${formatSpan(bucket.start, series.bucketMs)} · ${runsLabel(bucket.runs)} · ${tokensLabel(bucket.tokens)}`;
  return (
    <p className="th-usage-readout" aria-live="polite">
      {text}
    </p>
  );
}

function SeriesView({ series }: { readonly series: AgentUsageSeries }) {
  const [pick, setPick] = useState<Pick>(NO_PICK);
  const onPick: BarsProps["onPick"] = (change) =>
    setPick((prev) => ({ ...prev, ...(typeof change === "function" ? change(prev) : change) }));
  if (series.runs === 0) return <p className="th-cap">{`no runs in the last ${series.range}`}</p>;
  const shown = pick.hover ?? pick.focus ?? pick.pinned;
  const peak = (shares: readonly UsageShare[]) => Math.max(0, ...shares.map((s) => s.tokens));
  return (
    <>
      <Readout series={series} at={shown} />
      <Bars label="runs" unit={runsLabel} series={series} values={series.buckets.map((b) => b.runs)} pick={pick} onPick={onPick} />
      <Bars label="tokens" unit={tokensLabel} series={series} values={series.buckets.map((b) => b.tokens)} pick={pick} onPick={onPick} />
      <div className="th-usage-breakdown">
        <Shares title="by task" shares={series.byTask} peak={peak(series.byTask)} />
        <Shares title="by repo" shares={series.byRepo} peak={peak(series.byRepo)} />
        <Shares title="by conversation" shares={series.byConversation} peak={peak(series.byConversation)} />
      </div>
    </>
  );
}

export function UsageGraph({ name }: { readonly name: string }) {
  const [range, setRange] = useState<UsageRange>(DEFAULT_RANGE);
  const { data, error } = useAgentUsageSeries(name, range);
  return (
    <div className="th-usage">
      <div role="group" aria-label="usage range" className="th-usage-range">
        {RANGES.map((option) => (
          <Button key={option} size="sm" variant="ghost" aria-pressed={option === range} onClick={() => setRange(option)}>
            {option}
          </Button>
        ))}
      </div>
      {error !== null ? <Banner level="error">{`usage not loaded: ${error}`}</Banner> : null}
      {data === null && error === null ? <Loading verb="loading usage" /> : null}
      {data === null ? null : <SeriesView key={`${data.agent}:${data.range}`} series={data} />}
    </div>
  );
}
