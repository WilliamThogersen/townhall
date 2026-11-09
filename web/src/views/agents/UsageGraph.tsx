import { useMemo, useState } from "react";
import type { AgentUsageSeries, UsageRange, UsageShare } from "../../api/types";
import { useAgentUsageSeries, useConversationTitles } from "../../data";
import { Banner } from "../../ui/Banner";
import { Button } from "../../ui/Button";
import { Loading } from "../../ui/Loading";
import { compactNumber } from "../home/format";
import { conversationLabel } from "./conversationTitles";
import { axisTicks, barPercents, formatSpan, mergeBuckets, runsLabel, TARGET_BARS, tokensLabel } from "./usageSeries";

const RANGES: readonly UsageRange[] = ["24h", "7d", "30d"];
const DEFAULT_RANGE: UsageRange = "7d";
const AXIS_LABELS = 4;

type Metric = "runs" | "tokens";
const METRICS: readonly Metric[] = ["tokens", "runs"];
const UNITS: Readonly<Record<Metric, (n: number) => string>> = { runs: runsLabel, tokens: tokensLabel };

interface Pick {
  readonly hover: number | null;
  readonly focus: number | null;
  readonly pinned: number | null;
}

const NO_PICK: Pick = { hover: null, focus: null, pinned: null };

interface Wide {
  readonly buckets: AgentUsageSeries["buckets"];
  readonly bucketMs: number;
}

interface BarsProps {
  readonly metric: Metric;
  readonly range: UsageRange;
  readonly wide: Wide;
  readonly pick: Pick;
  readonly onPick: (change: Partial<Pick> | ((prev: Pick) => Partial<Pick>)) => void;
}

function Chart({ metric, range, wide, pick, onPick }: BarsProps) {
  const values = wide.buckets.map((b) => b[metric]);
  const heights = barPercents(values);
  const unit = UNITS[metric];
  const peak = Math.max(0, ...values);
  const ticks = axisTicks(wide.buckets, range, AXIS_LABELS);
  const count = wide.buckets.length;
  return (
    <div className="th-usage-chart">
      <span className="th-usage-y th-usage-y--max" data-testid="usage-y-max">
        {compactNumber(peak)}
      </span>
      <span className="th-usage-y th-usage-y--min">0</span>
      <div role="group" aria-label={`${metric} over time`} className="th-usage-bars" onMouseLeave={() => onPick({ hover: null })}>
        {wide.buckets.map((bucket, at) => {
          const span = formatSpan(bucket.start, wide.bucketMs);
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
      <div className="th-usage-axis" data-testid="usage-x-axis" aria-hidden="true">
        {ticks.map((tick) => (
          <span key={tick.at} className="th-usage-tick" style={{ left: `${((tick.at + 0.5) / count) * 100}%` }}>
            {tick.label}
          </span>
        ))}
      </div>
    </div>
  );
}

interface SharesProps {
  readonly title: string;
  readonly shares: readonly UsageShare[];
  readonly label?: (key: string) => string;
}

function Shares({ title, shares, label }: SharesProps) {
  const peak = Math.max(0, ...shares.map((s) => s.tokens));
  return (
    <div className="th-usage-shares">
      <h4 className="th-cap">{title}</h4>
      <ul aria-label={title} className="th-usage-share-list">
        {shares.map((share) => (
          <li key={share.key} className="th-usage-share" title={share.key}>
            <span className="th-usage-share-bar" style={{ width: `${peak === 0 ? 0 : (share.tokens / peak) * 100}%` }} />
            <span className="th-usage-share-key">{label === undefined ? share.key : label(share.key)}</span>
            <span className="th-usage-share-value">{`${tokensLabel(share.tokens)} · ${runsLabel(share.runs)}`}</span>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Readout({ series, wide, at }: { readonly series: AgentUsageSeries; readonly wide: Wide; readonly at: number | null }) {
  const bucket = at === null ? undefined : wide.buckets[at];
  const text =
    bucket === undefined
      ? `last ${series.range}: ${runsLabel(series.runs)} · ${tokensLabel(series.tokens)}`
      : `${formatSpan(bucket.start, wide.bucketMs)} · ${runsLabel(bucket.runs)} · ${tokensLabel(bucket.tokens)}`;
  return (
    <p className="th-usage-readout" aria-live="polite">
      {text}
    </p>
  );
}

function SeriesView({ series, metric }: { readonly series: AgentUsageSeries; readonly metric: Metric }) {
  const [pick, setPick] = useState<Pick>(NO_PICK);
  const titles = useConversationTitles();
  const wide = useMemo(() => mergeBuckets(series.buckets, series.bucketMs, TARGET_BARS[series.range]), [series]);
  const onPick: BarsProps["onPick"] = (change) =>
    setPick((prev) => ({ ...prev, ...(typeof change === "function" ? change(prev) : change) }));
  if (series.runs === 0) return <p className="th-cap">{`no runs in the last ${series.range}`}</p>;
  const shown = pick.hover ?? pick.focus ?? pick.pinned;
  return (
    <>
      <Readout series={series} wide={wide} at={shown} />
      <Chart metric={metric} range={series.range} wide={wide} pick={pick} onPick={onPick} />
      <div className="th-usage-breakdown">
        <Shares title="by task" shares={series.byTask} />
        <Shares title="by repo" shares={series.byRepo} />
        <Shares title="by conversation" shares={series.byConversation} label={(id) => conversationLabel(id, titles)} />
      </div>
    </>
  );
}

export function UsageGraph({ name }: { readonly name: string }) {
  const [range, setRange] = useState<UsageRange>(DEFAULT_RANGE);
  const [metric, setMetric] = useState<Metric>("tokens");
  const { data, error } = useAgentUsageSeries(name, range);
  return (
    <div className="th-usage">
      <div className="th-usage-controls">
        <div role="group" aria-label="usage range" className="th-usage-range">
          {RANGES.map((option) => (
            <Button key={option} size="sm" variant="ghost" aria-pressed={option === range} onClick={() => setRange(option)}>
              {option}
            </Button>
          ))}
        </div>
        <div role="group" aria-label="usage metric" className="th-usage-range">
          {METRICS.map((option) => (
            <Button key={option} size="sm" variant="ghost" aria-pressed={option === metric} onClick={() => setMetric(option)}>
              {option}
            </Button>
          ))}
        </div>
      </div>
      {error !== null ? <Banner level="error">{`usage not loaded: ${error}`}</Banner> : null}
      {data === null && error === null ? <Loading verb="loading usage" /> : null}
      {data === null ? null : <SeriesView key={`${data.agent}:${data.range}`} series={data} metric={metric} />}
    </div>
  );
}
