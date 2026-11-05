import type { ShippedDay } from "../../api/types";

export type MetricKey = Exclude<keyof ShippedDay, "date">;

export interface Metric {
  readonly key: MetricKey;
  readonly label: string;
  readonly one: string;
  readonly many: string;
  readonly verb: string;
  readonly sign: string;
}

export const METRICS: readonly Metric[] = [
  { key: "fixes", label: "bug fixes", one: "bug fix", many: "bug fixes", verb: "shipped", sign: "" },
  { key: "features", label: "features", one: "feature", many: "features", verb: "shipped", sign: "" },
  { key: "lines", label: "lines shipped", one: "line", many: "lines", verb: "shipped", sign: "+" },
  { key: "agents", label: "agents run", one: "agent", many: "agents", verb: "run", sign: "" },
];
