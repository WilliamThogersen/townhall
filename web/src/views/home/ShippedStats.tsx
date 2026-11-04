import { useMemo, useState } from "react";
import type { Stats } from "../../api/types";
import { useStats } from "../../data";
import { Segmented } from "../../ui/Segmented";
import { heatModel, periodPhrase } from "./heat";
import { Heatmap } from "./Heatmap";
import { METRICS, type Metric, type MetricKey } from "./metrics";
import { dailyValues, hasShipped, indexDays, periodDates, periodsPresent, periodYear, type DaysByDate } from "./shipped";
import { StatTiles } from "./StatTiles";
import { tileModels } from "./tiles";
import "./home.css";

export const STATS_TITLE = "shipped with townhall · last 12 months · all repos";
const STATS_LABEL = "Shipped with townhall";
const FIRST_METRIC = METRICS[0]!;

function StatsNote({ text }: { readonly text: string }) {
  return (
    <section className="th-stats" aria-label={STATS_LABEL}>
      <div className="th-stats-head">
        <h2>{STATS_TITLE}</h2>
      </div>
      <p className="th-stats-note">{text}</p>
    </section>
  );
}

function heatFor(today: Date, days: DaysByDate, period: number, metric: Metric) {
  const dates = periodDates(today, period);
  const when = periodPhrase(period, periodYear(today, period));
  return heatModel({ dates, values: dailyValues(dates, days, metric.key), metric, when });
}

function ShippedView({ stats, today }: { readonly stats: Stats; readonly today: Date }) {
  const [metricKey, setMetricKey] = useState<MetricKey>(FIRST_METRIC.key);
  const [chosenPeriod, setChosenPeriod] = useState(0);
  const days = useMemo(() => indexDays(stats.days), [stats]);
  const periods = useMemo(() => periodsPresent(today, days), [today, days]);
  const period = periods.includes(chosenPeriod) ? chosenPeriod : 0;
  const metric = METRICS.find((m) => m.key === metricKey) ?? FIRST_METRIC;
  const tiles = useMemo(() => tileModels(today, days, period), [today, days, period]);
  const heat = useMemo(() => heatFor(today, days, period, metric), [today, days, period, metric]);
  const options = periods.map((p) => ({ value: String(p), label: String(periodYear(today, p)) }));
  return (
    <section className="th-stats" aria-label={STATS_LABEL}>
      <div className="th-stats-head">
        <h2>{STATS_TITLE}</h2>
        <Segmented
          name="th-stats-period"
          label="Period"
          value={String(period)}
          options={options}
          onChange={(value) => setChosenPeriod(Number(value))}
        />
      </div>
      <StatTiles tiles={tiles} selected={metric.key} onSelect={setMetricKey} />
      <Heatmap model={heat} label={`${metric.many} by day`} />
    </section>
  );
}

export function ShippedStats({ today }: { readonly today: Date }) {
  const { stats, error } = useStats();
  if (stats === null) return error === null ? null : <StatsNote text={`error: ${error}`} />;
  if (!hasShipped(stats)) return <StatsNote text="nothing shipped yet" />;
  return <ShippedView stats={stats} today={today} />;
}
